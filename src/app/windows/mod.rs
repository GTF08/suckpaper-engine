use std::ptr::null_mut;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows_sys::core::{s, BOOL};
use windows_sys::Win32::Foundation::{HWND, LPARAM};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, FindWindowA, FindWindowExA, GetWindowLongPtrW, SendMessageTimeoutA,
    SetLayeredWindowAttributes, SetParent, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE,
    GWL_STYLE, LWA_ALPHA, SMTO_NORMAL, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SWP_SHOWWINDOW, WS_CHILD, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP,
};
use winit::window::Window;

fn get_hwnd(window: &Window) -> HWND {
    match window.window_handle().unwrap().as_raw() {
        RawWindowHandle::Win32(h) => h.hwnd.get() as HWND,
        _ => panic!("Поддерживается только Windows"),
    }
}

unsafe extern "system" fn enum_windows_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let shell = unsafe { FindWindowExA(hwnd, null_mut(), s!("SHELLDLL_DefView"), null_mut()) };
    if !shell.is_null() {
        let worker = unsafe { FindWindowExA(null_mut(), hwnd, s!("WorkerW"), null_mut()) };
        if !worker.is_null() {
            unsafe { *(lparam as *mut HWND) = worker };
            return 0;
        }
    }
    1
}

pub fn attach_to_desktop(window: &Window) {
    let hwnd = get_hwnd(window);
    let size = window.inner_size();

    unsafe {
        let progman = FindWindowA(s!("Progman"), null_mut());
        let mut res = 0;
        SendMessageTimeoutA(progman, 0x052C, 0xD, 0x1, SMTO_NORMAL, 1000, &mut res);

        let shell_in_progman = FindWindowExA(progman, null_mut(), s!("SHELLDLL_DefView"), null_mut());

        if !shell_in_progman.is_null() {
            // 1. В Win 11 24H2+ дочернее окно в Progman обязано иметь WS_EX_LAYERED
            let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            SetWindowLongPtrW(
                hwnd,
                GWL_EXSTYLE,
                ex_style | (WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE) as isize,
            );
            SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA);

            // 2. Делаем окно дочерним для Progman
            let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
            SetWindowLongPtrW(hwnd, GWL_STYLE, (style & !(WS_POPUP as isize)) | WS_CHILD as isize);
            SetParent(hwnd, progman);

            // 3. Ставим наше окно строго ПОД слой иконок (shell_in_progman)
            SetWindowPos(
                hwnd,
                shell_in_progman,
                0,
                0,
                size.width as i32,
                size.height as i32,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );

            // 4. Опускаем системный WorkerW ПОЗАДИ нашего окна (не скрывая его!)
            let worker_w = FindWindowExA(progman, null_mut(), s!("WorkerW"), null_mut());
            if !worker_w.is_null() {
                SetWindowPos(
                    worker_w,
                    hwnd,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE,
                );
            }
        } else {
            // Классическая Windows 10
            let mut worker_w: HWND = null_mut();
            EnumWindows(Some(enum_windows_proc), &mut worker_w as *mut HWND as LPARAM);
            if !worker_w.is_null() {
                SetParent(hwnd, worker_w);
                SetWindowPos(
                    hwnd,
                    null_mut(),
                    0,
                    0,
                    size.width as i32,
                    size.height as i32,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW,
                );
            }
        }
    }
}
