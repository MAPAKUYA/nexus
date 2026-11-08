//! Получение данных от Windows. Собираются только имена исполняемых файлов и их описание из
//! свойств файла: заголовки окон, содержимое окон и ввод пользователя не читаются и не сохраняются.

use nexus_core::Sample;
use std::{
    collections::{HashMap, HashSet},
    ffi::c_void,
    mem::size_of,
    sync::OnceLock,
};
use windows_sys::Win32::{
    Foundation::{
        CloseHandle, GetLastError, BOOL, ERROR_ACCESS_DENIED, ERROR_ALREADY_EXISTS, ERROR_FILE_NOT_FOUND, HWND, LPARAM,
        LRESULT, SYSTEMTIME, WPARAM,
    },
    Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED},
    Graphics::Gdi::{
        CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, BITMAP, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
        DIB_RGB_COLORS,
    },
    Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW},
    System::{
        LibraryLoader::GetModuleHandleW,
        Registry::{
            RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
            KEY_READ, KEY_SET_VALUE, REG_DWORD, REG_SZ,
        },
        StationsAndDesktops::{CloseDesktop, OpenInputDesktop, DESKTOP_SWITCHDESKTOP},
        SystemInformation::{GetLocalTime, GetTickCount},
        Threading::{
            CreateMutexW, OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
        },
    },
    UI::{
        Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO},
        Shell::ExtractIconExW,
        WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DestroyIcon, DispatchMessageW, EnumChildWindows, GetIconInfo, HICON, ICONINFO, EnumWindows, FindWindowW, PostMessageW,
            GetForegroundWindow, GetMessageW, GetWindow, GetWindowLongW, GetWindowTextLengthW,
            GetWindowThreadProcessId, IsWindowVisible, RegisterClassW, TranslateMessage, GWL_EXSTYLE, GW_OWNER, MSG,
            WM_APP, WM_ENDSESSION, WM_QUERYENDSESSION,
            WNDCLASSW, WS_EX_TOOLWINDOW,
        },
    },
};

/// Список запущенных приложений обновляется раз в N замеров (замеры идут раз в секунду).
const RUNNING_REFRESH_TICKS: u32 = 5;

/// Служебные процессы оболочки Windows: у них бывают видимые окна, но это не приложения пользователя.
const IGNORED_EXES: [&str; 5] = [
    "textinputhost.exe",
    "shellexperiencehost.exe",
    "searchhost.exe",
    "startmenuexperiencehost.exe",
    "applicationframehost.exe", // сам хост; настоящее UWP-приложение определяется по дочернему окну
];
const UWP_HOST: &str = "applicationframehost.exe";

pub struct Sampler {
    own_pid: u32,
    tick: u32,
    running: Vec<String>,
    pid_cache: HashMap<u32, Option<String>>,
    known_names: HashSet<String>,
    new_names: Vec<(String, String)>,
    /// Нужны ли иконки (настройка «Показывать иконки»). Пока выключено, иконки не извлекаются вообще.
    want_icons: bool,
    /// exe, для которых иконку уже пробовали получить в этом запуске (успешно или нет).
    icon_tried: HashSet<String>,
    new_icons: Vec<(String, Vec<u8>)>,
}

impl Sampler {
    pub fn new() -> Self {
        Self {
            own_pid: std::process::id(),
            tick: 0,
            running: Vec::new(),
            pid_cache: HashMap::new(),
            known_names: HashSet::new(),
            new_names: Vec::new(),
            want_icons: false,
            icon_tried: HashSet::new(),
            new_icons: Vec::new(),
        }
    }

    /// Включает/выключает получение иконок. При включении иконки запрашиваются заново для всех
    /// приложений, которые видны в ближайших замерах.
    pub fn set_want_icons(&mut self, want: bool) {
        if want && !self.want_icons {
            self.icon_tried.clear();
        }
        self.want_icons = want;
    }

    /// Новые иконки приложений (PNG), найденные с прошлого вызова: `(exe, png)`.
    pub fn take_new_icons(&mut self) -> Vec<(String, Vec<u8>)> {
        std::mem::take(&mut self.new_icons)
    }

    /// Читаемые имена приложений, найденные с прошлого вызова: `(exe, имя)`.
    pub fn take_new_names(&mut self) -> Vec<(String, String)> {
        std::mem::take(&mut self.new_names)
    }

    pub fn sample(&mut self) -> Sample {
        if self.tick % RUNNING_REFRESH_TICKS == 0 {
            // Кэш pid -> exe сбрасываем при каждом обновлении, чтобы повторное использование pid
            // не приводило к ошибочной атрибуции дольше чем на несколько секунд.
            self.pid_cache.clear();
            self.refresh_running();
        }
        self.tick = self.tick.wrapping_add(1);

        // SAFETY: простой вызов Win32 без владения ресурсами.
        let fg_hwnd = unsafe { GetForegroundWindow() };
        let fg_exe = if fg_hwnd.is_null() { None } else { self.exe_of_window(fg_hwnd, false) };
        // Экран входа/блокировки Windows 10/11 — это lockapp.exe в сессии пользователя.
        let locked = is_workstation_locked() || fg_exe.as_deref() == Some("lockapp.exe");
        let foreground = fg_exe.filter(|e| !IGNORED_EXES.contains(&e.as_str()));

        Sample { day: local_day(), foreground, running: self.running.clone(), idle_ms: input_idle_ms(), locked }
    }

    fn refresh_running(&mut self) {
        let mut windows: Vec<isize> = Vec::new();
        // SAFETY: колбэк вызывается синхронно внутри EnumWindows, указатель на `windows` живёт дольше.
        unsafe {
            EnumWindows(Some(enum_proc), &mut windows as *mut Vec<isize> as LPARAM);
        }
        let mut exes: Vec<String> = windows
            .into_iter()
            .filter_map(|h| self.exe_of_window(h as HWND, true))
            .filter(|e| !IGNORED_EXES.contains(&e.as_str()))
            .collect();
        exes.sort_unstable();
        exes.dedup();
        self.running = exes;
    }

    /// exe, которому принадлежит окно. Окна магазинных приложений принадлежат ApplicationFrameHost,
    /// поэтому настоящий процесс ищется среди дочерних окон.
    fn exe_of_window(&mut self, hwnd: HWND, _top_level: bool) -> Option<String> {
        let mut pid = 0u32;
        // SAFETY: hwnd получен от системы; pid — локальная переменная.
        unsafe {
            GetWindowThreadProcessId(hwnd, &mut pid);
        }
        if pid == 0 || pid == self.own_pid {
            return None;
        }
        let exe = self.exe_for(pid)?;
        if exe == UWP_HOST {
            return child_pid(hwnd, pid).filter(|&p| p != self.own_pid).and_then(|p| self.exe_for(p));
        }
        Some(exe)
    }

    fn exe_for(&mut self, pid: u32) -> Option<String> {
        if let Some(cached) = self.pid_cache.get(&pid) {
            return cached.clone();
        }
        let found = query_exe(pid);
        let exe = found.as_ref().map(|(exe, _)| exe.clone());
        if let Some((exe, path)) = found {
            if self.want_icons && self.icon_tried.insert(exe.clone()) {
                if let Some(png) = icon_png(&path) {
                    self.new_icons.push((exe.clone(), png));
                }
            }
            if self.known_names.insert(exe.clone()) {
                if let Some(name) = friendly_name(&path) {
                    self.new_names.push((exe, name));
                }
            }
        }
        self.pid_cache.insert(pid, exe.clone());
        exe
    }
}

/// Локальная дата в формате `YYYY-MM-DD`.
pub fn local_day() -> String {
    // SAFETY: SYSTEMTIME — простая структура из чисел, нулевая инициализация допустима.
    let st: SYSTEMTIME = unsafe {
        let mut st: SYSTEMTIME = std::mem::zeroed();
        GetLocalTime(&mut st);
        st
    };
    format!("{:04}-{:02}-{:02}", st.wYear, st.wMonth, st.wDay)
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// `(имя exe в нижнем регистре, полный путь)`.
fn query_exe(pid: u32) -> Option<(String, String)> {
    // SAFETY: хэндл открывается и сразу закрывается; буфер локальный.
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return None;
        }
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(handle, 0, buf.as_mut_ptr(), &mut len);
        CloseHandle(handle);
        if ok == 0 {
            return None;
        }
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        let exe = path.rsplit('\\').next().map(|s| s.to_lowercase()).filter(|s| !s.is_empty())?;
        Some((exe, path))
    }
}

/// Иконка программы как PNG: большая иконка из самого exe-файла (у файла без иконки — `None`).
/// Читается только изображение иконки, ничего больше.
fn icon_png(path: &str) -> Option<Vec<u8>> {
    let path_w = wide(path);
    let mut large: HICON = std::ptr::null_mut();
    // SAFETY: путь — нуль-терминированная строка; в `large` система кладёт хэндл иконки, который
    // мы обязаны освободить (DestroyIcon ниже).
    unsafe {
        ExtractIconExW(path_w.as_ptr(), 0, &mut large, std::ptr::null_mut(), 1);
    }
    if large.is_null() {
        return None;
    }
    let png = icon_to_png(large);
    // SAFETY: хэндл получен выше и больше не используется.
    unsafe {
        DestroyIcon(large);
    }
    png
}

fn icon_to_png(icon: HICON) -> Option<Vec<u8>> {
    // SAFETY: ICONINFO — набор чисел и хэндлов, нулевая инициализация допустима.
    let mut info: ICONINFO = unsafe { std::mem::zeroed() };
    // SAFETY: `icon` — действующий хэндл; `info` — локальная структура.
    if unsafe { GetIconInfo(icon, &mut info) } == 0 {
        return None;
    }
    let (color, mask) = (info.hbmColor, info.hbmMask);
    let png = bitmap_to_png(color, mask);
    // SAFETY: GetIconInfo передаёт владение обеими картинками нам; освобождаем каждую один раз.
    unsafe {
        if !color.is_null() {
            DeleteObject(color);
        }
        if !mask.is_null() {
            DeleteObject(mask);
        }
    }
    png
}

/// Пиксели цветной картинки иконки (и её маски прозрачности) → PNG. Одноцветные иконки без
/// цветной картинки пропускаются.
fn bitmap_to_png(color: *mut c_void, mask: *mut c_void) -> Option<Vec<u8>> {
    if color.is_null() {
        return None;
    }
    // SAFETY: BITMAP — набор чисел, нулевая инициализация допустима; `color` — действующая картинка.
    let (w, h) = unsafe {
        let mut bm: BITMAP = std::mem::zeroed();
        if GetObjectW(color, size_of::<BITMAP>() as i32, &mut bm as *mut BITMAP as *mut c_void) == 0 {
            return None;
        }
        (bm.bmWidth, bm.bmHeight)
    };
    if w <= 0 || h <= 0 || w as u32 > nexus_core::MAX_ICON_SIDE || h as u32 > nexus_core::MAX_ICON_SIDE {
        return None;
    }
    let len = w as usize * h as usize * 4;
    // SAFETY: контекст создаётся и освобождается здесь; буферы — локальные векторы нужной длины,
    // GetDIBits пишет в них не больше `h` строк по `w * 4` байт (32 бита на точку, сверху вниз).
    let (bgra, mask_px) = unsafe {
        let hdc = CreateCompatibleDC(std::ptr::null_mut());
        if hdc.is_null() {
            return None;
        }
        let mut bmi: BITMAPINFO = std::mem::zeroed();
        bmi.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = w;
        bmi.bmiHeader.biHeight = -h; // отрицательная высота — строки идут сверху вниз
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = BI_RGB;
        let mut bgra = vec![0u8; len];
        let got = GetDIBits(hdc, color, 0, h as u32, bgra.as_mut_ptr() as *mut c_void, &mut bmi, DIB_RGB_COLORS);
        let mut mask_px = None;
        if got != 0 && !mask.is_null() {
            let mut m = vec![0u8; len];
            if GetDIBits(hdc, mask, 0, h as u32, m.as_mut_ptr() as *mut c_void, &mut bmi, DIB_RGB_COLORS) != 0 {
                mask_px = Some(m);
            }
        }
        DeleteDC(hdc);
        if got == 0 {
            return None;
        }
        (bgra, mask_px)
    };
    let rgba = nexus_core::bgra_to_rgba(&bgra, mask_px.as_deref());
    nexus_core::encode_png_rgba(w as u32, h as u32, &rgba)
}

/// Читаемое имя программы из свойств файла (описание или название продукта).
fn friendly_name(path: &str) -> Option<String> {
    let path_w = wide(path);
    // SAFETY: все буферы локальные и живут дольше вызовов; размеры берутся из ответов системы.
    unsafe {
        let mut handle = 0u32;
        let size = GetFileVersionInfoSizeW(path_w.as_ptr(), &mut handle);
        if size == 0 {
            return None;
        }
        let mut block = vec![0u8; size as usize];
        if GetFileVersionInfoW(path_w.as_ptr(), 0, size, block.as_mut_ptr() as *mut c_void) == 0 {
            return None;
        }

        let mut langs: Vec<(u16, u16)> = Vec::new();
        let mut ptr: *mut c_void = std::ptr::null_mut();
        let mut len = 0u32;
        let key = wide("\\VarFileInfo\\Translation");
        if VerQueryValueW(block.as_ptr() as *const c_void, key.as_ptr(), &mut ptr, &mut len) != 0 && len >= 4 {
            let mut raw = vec![0u16; (len / 2) as usize];
            std::ptr::copy_nonoverlapping(ptr as *const u8, raw.as_mut_ptr() as *mut u8, raw.len() * 2);
            langs.extend(raw.chunks_exact(2).map(|c| (c[0], c[1])));
        }
        langs.extend([(0x0409, 0x04B0), (0x0409, 0x04E4), (0x0419, 0x04B0)]);

        for field in ["FileDescription", "ProductName"] {
            for (lang, code) in &langs {
                let sub = wide(&format!("\\StringFileInfo\\{lang:04x}{code:04x}\\{field}"));
                let (mut p, mut l) = (std::ptr::null_mut::<c_void>(), 0u32);
                if VerQueryValueW(block.as_ptr() as *const c_void, sub.as_ptr(), &mut p, &mut l) != 0 && l > 0 {
                    let mut text = vec![0u16; l as usize];
                    std::ptr::copy_nonoverlapping(p as *const u8, text.as_mut_ptr() as *mut u8, text.len() * 2);
                    if let Some(clean) = clean_name(&String::from_utf16_lossy(&text)) {
                        return Some(clean);
                    }
                }
            }
        }
        None
    }
}

/// Убирает управляющие символы и пробелы, ограничивает длину.
fn clean_name(raw: &str) -> Option<String> {
    let first_line = raw.trim_matches(|c: char| c == '\0' || c.is_whitespace()).lines().next()?;
    let cleaned: String = first_line.chars().filter(|c| !c.is_control()).take(60).collect();
    let cleaned = cleaned.trim().to_string();
    (!cleaned.is_empty()).then_some(cleaned)
}

struct ChildSearch {
    host_pid: u32,
    found: u32,
}

unsafe extern "system" fn child_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let search = &mut *(lparam as *mut ChildSearch);
    let mut pid = 0u32;
    GetWindowThreadProcessId(hwnd, &mut pid);
    if pid != 0 && pid != search.host_pid {
        search.found = pid;
        return 0; // нашли — остановить перечисление
    }
    1
}

fn child_pid(hwnd: HWND, host_pid: u32) -> Option<u32> {
    let mut search = ChildSearch { host_pid, found: 0 };
    // SAFETY: колбэк синхронный, `search` живёт дольше вызова.
    unsafe {
        EnumChildWindows(hwnd, Some(child_proc), &mut search as *mut ChildSearch as LPARAM);
    }
    (search.found != 0).then_some(search.found)
}

unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let windows = &mut *(lparam as *mut Vec<isize>);
    if is_app_window(hwnd) {
        windows.push(hwnd as isize);
    }
    1 // продолжить перечисление
}

/// Окно «настоящего» приложения: видимое, без владельца, не tool-window, с заголовком, не скрытое.
/// Сам текст заголовка не читается — проверяется только его длина.
unsafe fn is_app_window(hwnd: HWND) -> bool {
    if IsWindowVisible(hwnd) == 0 || !GetWindow(hwnd, GW_OWNER).is_null() {
        return false;
    }
    if (GetWindowLongW(hwnd, GWL_EXSTYLE) as u32) & WS_EX_TOOLWINDOW != 0 {
        return false;
    }
    if GetWindowTextLengthW(hwnd) == 0 {
        return false;
    }
    // «Cloaked»-окна (UWP-призраки, окна на других рабочих столах) не считаем.
    let mut cloaked: u32 = 0;
    let hr = DwmGetWindowAttribute(
        hwnd,
        DWMWA_CLOAKED as u32,
        &mut cloaked as *mut u32 as *mut c_void,
        size_of::<u32>() as u32,
    );
    !(hr >= 0 && cloaked != 0)
}

/// Сколько мс прошло с последнего ввода. Читается только момент ввода, но не его содержимое.
fn input_idle_ms() -> u64 {
    // SAFETY: структура инициализирована с корректным размером; вызовы не имеют побочных эффектов.
    unsafe {
        let mut info = LASTINPUTINFO { cbSize: size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
        if GetLastInputInfo(&mut info) == 0 {
            return 0; // не удалось определить — считаем, что пользователь на месте
        }
        // Оба значения — 32-битные тики, переполнение корректно обрабатывается wrapping_sub.
        GetTickCount().wrapping_sub(info.dwTime) as u64
    }
}

/// Рабочая станция заблокирована (или показан защищённый рабочий стол, например запрос UAC):
/// обычный процесс не может открыть «входной» рабочий стол с правом переключения.
fn is_workstation_locked() -> bool {
    // SAFETY: хэндл рабочего стола сразу закрывается.
    unsafe {
        let desktop = OpenInputDesktop(0, 0, DESKTOP_SWITCHDESKTOP);
        if desktop.is_null() {
            GetLastError() == ERROR_ACCESS_DENIED
        } else {
            CloseDesktop(desktop);
            false
        }
    }
}

/// Возвращает `true`, если это первый запущенный экземпляр. Два трекера, пишущих в одну базу,
/// удвоили бы время, поэтому второй экземпляр запускаться не должен.
pub fn acquire_single_instance() -> bool {
    let name = wide("Local\\NEXUS-Tracker-SingleInstance");
    // SAFETY: имя — корректная нуль-терминированная UTF-16 строка. Хэндл намеренно не закрывается:
    // мьютекс должен жить до завершения процесса.
    unsafe {
        let handle = CreateMutexW(std::ptr::null(), 0, name.as_ptr());
        !handle.is_null() && GetLastError() != ERROR_ALREADY_EXISTS
    }
}

/// Просит уже работающий экземпляр показать своё окно. Окна может не быть (приложение в трее),
/// поэтому сообщение отправляется скрытому окну-наблюдателю, которое есть всегда.
pub fn request_show_existing() {
    let class = wide(WATCHER_CLASS);
    // SAFETY: строка корректна и нуль-терминирована; PostMessageW только ставит сообщение в очередь.
    unsafe {
        let hwnd = FindWindowW(class.as_ptr(), std::ptr::null());
        if !hwnd.is_null() {
            PostMessageW(hwnd, WM_APP_SHOW, 0, 0);
        }
    }
}

// ——— Автозапуск (только раздел текущего пользователя в реестре, права администратора не нужны) ———

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const RUN_VALUE: &str = "NEXUS";

fn open_run_key(access: u32) -> Option<HKEY> {
    let sub = wide(RUN_KEY);
    let mut key: HKEY = std::ptr::null_mut();
    // SAFETY: корректная нуль-терминированная строка и указатель на локальную переменную.
    let rc = unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, sub.as_ptr(), 0, access, &mut key) };
    (rc == 0).then_some(key)
}

pub fn autostart_enabled() -> bool {
    let Some(key) = open_run_key(KEY_READ) else { return false };
    let name = wide(RUN_VALUE);
    let mut size = 0u32;
    // SAFETY: запрашивается только размер значения.
    let rc = unsafe {
        RegQueryValueExW(key, name.as_ptr(), std::ptr::null(), std::ptr::null_mut(), std::ptr::null_mut(), &mut size)
    };
    // SAFETY: ключ открыт выше.
    unsafe { RegCloseKey(key) };
    rc == 0
}

/// Включает/выключает запуск вместе с Windows (свёрнутым в трей).
pub fn set_autostart(enabled: bool) -> Result<(), String> {
    let key = open_run_key(KEY_SET_VALUE).ok_or("не удалось открыть раздел автозапуска")?;
    let name = wide(RUN_VALUE);
    let result = if enabled {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let command = wide(&format!("\"{}\" --minimized", exe.display()));
        // SAFETY: данные — корректная UTF-16 строка; размер передаётся в байтах.
        let rc = unsafe {
            RegSetValueExW(key, name.as_ptr(), 0, REG_SZ, command.as_ptr() as *const u8, (command.len() * 2) as u32)
        };
        if rc == 0 { Ok(()) } else { Err(format!("не удалось включить автозапуск (код {rc})")) }
    } else {
        // SAFETY: ключ открыт выше.
        let rc = unsafe { RegDeleteValueW(key, name.as_ptr()) };
        if rc == 0 || rc == ERROR_FILE_NOT_FOUND { Ok(()) } else { Err(format!("не удалось выключить автозапуск (код {rc})")) }
    };
    // SAFETY: ключ открыт выше.
    unsafe { RegCloseKey(key) };
    result
}

/// Язык, выбранный при установке (`HKCU\Software\NEXUS\Language`, пишет установщик).
/// У портативной версии значения нет — тогда используется английский.
pub fn installer_language() -> Option<String> {
    let sub = wide("Software\\NEXUS");
    let mut key: HKEY = std::ptr::null_mut();
    // SAFETY: корректные строки и указатели на локальные переменные; ключ закрывается ниже.
    let rc = unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, sub.as_ptr(), 0, KEY_READ, &mut key) };
    if rc != 0 {
        return None;
    }
    let name = wide("Language");
    let mut buf = [0u16; 16];
    let mut size = (buf.len() * 2) as u32;
    let mut kind = 0u32;
    // SAFETY: буфер локальный, его размер в байтах передаётся в `size`.
    let rc = unsafe {
        RegQueryValueExW(key, name.as_ptr(), std::ptr::null(), &mut kind, buf.as_mut_ptr() as *mut u8, &mut size)
    };
    // SAFETY: ключ открыт выше.
    unsafe { RegCloseKey(key) };
    if rc != 0 || kind != REG_SZ {
        return None;
    }
    let len = (size as usize / 2).min(buf.len());
    Some(String::from_utf16_lossy(&buf[..len]).trim_end_matches('\0').trim().to_string())
}

/// Светлая ли тема приложений в Windows (`HKCU\...\Themes\Personalize\AppsUseLightTheme`: 0 — тёмная, 1 — светлая).
/// Если значения нет (старые сборки Windows 10), считается светлой — как и у WebView2.
pub fn system_prefers_light() -> bool {
    let sub = wide("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");
    let mut key: HKEY = std::ptr::null_mut();
    // SAFETY: корректная нуль-терминированная строка и указатель на локальную переменную.
    let rc = unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, sub.as_ptr(), 0, KEY_READ, &mut key) };
    if rc != 0 {
        return true;
    }
    let name = wide("AppsUseLightTheme");
    let mut value = 1u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    let mut kind = 0u32;
    // SAFETY: буфер — локальная переменная u32, его размер в байтах передаётся в `size`.
    let rc = unsafe {
        RegQueryValueExW(key, name.as_ptr(), std::ptr::null(), &mut kind, &mut value as *mut u32 as *mut u8, &mut size)
    };
    // SAFETY: ключ открыт выше.
    unsafe { RegCloseKey(key) };
    if rc != 0 || kind != REG_DWORD {
        return true;
    }
    value != 0
}

// ——— Завершение сеанса Windows ———

const WATCHER_CLASS: &str = "NexusSessionWatcher";
/// Приватное сообщение «покажи окно» от повторно запущенного экземпляра.
const WM_APP_SHOW: u32 = WM_APP + 1;

static SESSION_END: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();
static SHOW_REQUEST: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();

unsafe extern "system" fn session_wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        // Сеанс завершается (выключение, перезагрузка, выход из учётной записи). Сохраняем данные
        // сразу при запросе: система даёт процессу считанные секунды. Повторный сброс безвреден.
        WM_QUERYENDSESSION | WM_ENDSESSION => {
            if let Some(cb) = SESSION_END.get() {
                cb();
            }
            if msg == WM_QUERYENDSESSION {
                1 // не мешаем завершению сеанса
            } else {
                0
            }
        }
        WM_APP_SHOW => {
            if let Some(cb) = SHOW_REQUEST.get() {
                cb();
            }
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Запускает поток со скрытым окном. Оно получает уведомления о завершении сеанса Windows
/// (`on_end` — немедленное сохранение статистики) и запросы от повторного запуска (`on_show`).
pub fn spawn_session_watcher(on_end: impl Fn() + Send + Sync + 'static, on_show: impl Fn() + Send + Sync + 'static) {
    let _ = SESSION_END.set(Box::new(on_end));
    let _ = SHOW_REQUEST.set(Box::new(on_show));
    let spawned = std::thread::Builder::new().name("nexus-session".into()).spawn(|| {
        // SAFETY: стандартный цикл сообщений скрытого окна; все указатели живут до конца потока.
        unsafe {
            let hinstance = GetModuleHandleW(std::ptr::null());
            let class = wide(WATCHER_CLASS);
            let wc = WNDCLASSW {
                style: 0,
                lpfnWndProc: Some(session_wndproc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: hinstance,
                hIcon: std::ptr::null_mut(),
                hCursor: std::ptr::null_mut(),
                hbrBackground: std::ptr::null_mut(),
                lpszMenuName: std::ptr::null(),
                lpszClassName: class.as_ptr(),
            };
            if RegisterClassW(&wc) == 0 {
                return;
            }
            // Верхнеуровневое (не message-only) невидимое окно — только такие получают WM_QUERYENDSESSION.
            let hwnd = CreateWindowExW(
                0,
                class.as_ptr(),
                class.as_ptr(),
                0,
                0,
                0,
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                hinstance,
                std::ptr::null(),
            );
            if hwnd.is_null() {
                return;
            }
            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    });
    if let Err(e) = spawned {
        eprintln!("NEXUS: не удалось запустить наблюдатель сеанса: {e}");
    }
}
