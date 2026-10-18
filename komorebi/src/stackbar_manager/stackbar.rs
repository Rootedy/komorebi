use crate::DEFAULT_CONTAINER_PADDING;
use crate::DATA_DIR;
use crate::WINDOWS_11;
use crate::WindowsApi;
use crate::border_manager::BORDER_OFFSET;
use crate::border_manager::BORDER_WIDTH;
use crate::border_manager::STYLE;
use crate::container::Container;
use crate::core::BorderStyle;
use crate::core::Rect;
use crate::core::StackbarLabel;
use crate::stackbar_manager::STACKBAR_FOCUSED_TEXT_COLOUR;
use crate::stackbar_manager::STACKBAR_FONT_FAMILY;
use crate::stackbar_manager::STACKBAR_FONT_SIZE;
use crate::stackbar_manager::STACKBAR_LABEL;
use crate::stackbar_manager::STACKBAR_TAB_BACKGROUND_COLOUR;
use crate::stackbar_manager::STACKBAR_TAB_HEIGHT;
use crate::stackbar_manager::STACKBAR_TAB_WIDTH;
use crate::stackbar_manager::STACKBAR_UNFOCUSED_TEXT_COLOUR;
use crate::stackbar_manager::STACKBARS_CONTAINERS;
use crate::core::SocketMessage;
use crate::windows_api;
use crossbeam_utils::atomic::AtomicConsume;
use std::io::Write;
use std::os::windows::ffi::OsStrExt;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::Duration;
use uds_windows::UnixStream;
use windows::Win32::Foundation::COLORREF;
use windows::Win32::Foundation::HINSTANCE;
use windows::Win32::Foundation::HWND;
use windows::Win32::Foundation::LPARAM;
use windows::Win32::Foundation::LRESULT;
use windows::Win32::Foundation::POINT;
use windows::Win32::Foundation::WPARAM;
use windows::Win32::Graphics::Gdi::CreateFontIndirectW;
use windows::Win32::Graphics::Gdi::CreatePen;
use windows::Win32::Graphics::Gdi::CreateSolidBrush;
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::Graphics::Gdi::DT_END_ELLIPSIS;
use windows::Win32::Graphics::Gdi::DT_SINGLELINE;
use windows::Win32::Graphics::Gdi::DT_VCENTER;
use windows::Win32::Graphics::Gdi::DeleteObject;
use windows::Win32::Graphics::Gdi::DrawTextW;
use windows::Win32::Graphics::Gdi::FONT_QUALITY;
use windows::Win32::Graphics::Gdi::FW_BOLD;
use windows::Win32::Graphics::Gdi::GetDC;
use windows::Win32::Graphics::Gdi::GetDeviceCaps;
use windows::Win32::Graphics::Gdi::LOGFONTW;
use windows::Win32::Graphics::Gdi::LOGPIXELSY;
use windows::Win32::Graphics::Gdi::PROOF_QUALITY;
use windows::Win32::Graphics::Gdi::PS_SOLID;
use windows::Win32::Graphics::Gdi::Rectangle;
use windows::Win32::Graphics::Gdi::ReleaseDC;
use windows::Win32::Graphics::Gdi::RoundRect;
use windows::Win32::Graphics::Gdi::SelectObject;
use windows::Win32::Graphics::Gdi::SetBkColor;
use windows::Win32::Graphics::Gdi::SetTextColor;
use windows::Win32::System::WindowsProgramming::MulDiv;
use windows::Win32::UI::WindowsAndMessaging::CS_HREDRAW;
use windows::Win32::UI::WindowsAndMessaging::CS_VREDRAW;
use windows::Win32::UI::WindowsAndMessaging::CreateWindowExW;
use windows::Win32::UI::WindowsAndMessaging::DefWindowProcW;
use windows::Win32::UI::WindowsAndMessaging::DispatchMessageW;
use windows::Win32::UI::WindowsAndMessaging::GetMessageW;
use windows::Win32::UI::WindowsAndMessaging::IDC_ARROW;
use windows::Win32::UI::WindowsAndMessaging::LWA_COLORKEY;
use windows::Win32::UI::WindowsAndMessaging::LoadCursorW;
use windows::Win32::UI::WindowsAndMessaging::MSG;
use windows::Win32::UI::WindowsAndMessaging::PostQuitMessage;
use windows::Win32::UI::WindowsAndMessaging::SetCursor;
use windows::Win32::UI::WindowsAndMessaging::SetLayeredWindowAttributes;
use windows::Win32::UI::WindowsAndMessaging::TranslateMessage;
use windows::Win32::UI::WindowsAndMessaging::WM_DESTROY;
use windows::Win32::UI::WindowsAndMessaging::WM_LBUTTONDOWN;
use windows::Win32::UI::WindowsAndMessaging::WM_RBUTTONUP;
use windows::Win32::UI::WindowsAndMessaging::AppendMenuW;
use windows::Win32::UI::WindowsAndMessaging::CreatePopupMenu;
use windows::Win32::UI::WindowsAndMessaging::DestroyMenu;
use windows::Win32::UI::WindowsAndMessaging::DI_NORMAL;
use windows::Win32::UI::WindowsAndMessaging::DrawIconEx;
use windows::Win32::UI::WindowsAndMessaging::GCLP_HICON;
use windows::Win32::UI::WindowsAndMessaging::GetClassLongPtrW;
use windows::Win32::UI::WindowsAndMessaging::HICON;
use windows::Win32::UI::WindowsAndMessaging::ICON_BIG;
use windows::Win32::UI::WindowsAndMessaging::ICON_SMALL;
use windows::Win32::UI::WindowsAndMessaging::MF_STRING;
use windows::Win32::UI::WindowsAndMessaging::SMTO_ABORTIFHUNG;
use windows::Win32::UI::WindowsAndMessaging::SendMessageTimeoutW;
use windows::Win32::UI::WindowsAndMessaging::TPM_NONOTIFY;
use windows::Win32::UI::WindowsAndMessaging::TPM_RETURNCMD;
use windows::Win32::UI::WindowsAndMessaging::TPM_RIGHTBUTTON;
use windows::Win32::UI::WindowsAndMessaging::TrackPopupMenuEx;
use windows::Win32::UI::WindowsAndMessaging::WM_GETICON;
use windows::Win32::UI::WindowsAndMessaging::WM_SETCURSOR;
use windows::Win32::UI::WindowsAndMessaging::WNDCLASSW;
use windows::Win32::UI::WindowsAndMessaging::WS_EX_LAYERED;
use windows::Win32::UI::WindowsAndMessaging::WS_EX_TOOLWINDOW;
use windows::Win32::UI::WindowsAndMessaging::WS_POPUP;
use windows::Win32::UI::WindowsAndMessaging::WS_VISIBLE;
use windows::core::PCWSTR;

#[derive(Debug)]
pub struct Stackbar {
    pub hwnd: isize,
}

/// custom: returns the stackbar tab index under the given client x coordinate
fn tab_index_at(container: &Container, x: i32) -> Option<usize> {
    let width = STACKBAR_TAB_WIDTH.load_consume();
    let gap = DEFAULT_CONTAINER_PADDING.load_consume();

    for (index, _) in container.windows().iter().enumerate() {
        let left = gap + (index as i32 * (width + gap));
        let right = left + width;

        if x >= left && x <= right {
            return Some(index);
        }
    }

    None
}

/// custom: focuses (restores, positions and focuses) the window corresponding to the
/// stackbar tab at the given index, hiding the other windows in the stack
unsafe fn focus_tab(container: &Container, index: usize) {
    let focused_window_idx = container.focused_window_idx();
    let focused_window_rect = WindowsApi::window_rect(
        container.focused_window().cloned().unwrap_or_default().hwnd,
    )
    .unwrap_or_default();

    for (i, window) in container.windows().iter().enumerate() {
        if i == index {
            // If we are focusing a window that isn't currently focused in the
            // stackbar, make sure we update its location so that it doesn't
            // render on top of other tiles before eventually ending up in the
            // correct tile
            if i != focused_window_idx
                && let Err(err) = window.set_position(&focused_window_rect, false)
            {
                tracing::error!(
                    "stackbar focus_tab repositioning error: hwnd {} ({})",
                    *window,
                    err
                );
            }

            window.restore_with_border(false);
            if let Err(err) = window.focus(false) {
                tracing::error!("stackbar focus_tab focus error: hwnd {} ({})", *window, err);
            }
        } else {
            // Hide any windows in the stack that don't correspond to the window
            // we have focused
            window.hide_with_border(false);
        }
    }
}

/// custom: retrieves the application icon for the given window, trying WM_GETICON
/// (small, then large) first and falling back to the window class icon
unsafe fn window_icon(hwnd: isize) -> Option<HICON> {
    let hwnd_h = HWND(windows_api::as_ptr!(hwnd));

    for flag in [ICON_SMALL, ICON_BIG] {
        let mut result: usize = 0;
        unsafe {
            SendMessageTimeoutW(
                hwnd_h,
                WM_GETICON,
                WPARAM(flag as usize),
                LPARAM(0),
                SMTO_ABORTIFHUNG,
                100,
                Some(&mut result),
            );
        }

        if result != 0 {
            return Some(HICON(result as *mut _));
        }
    }

    let class_icon = unsafe { GetClassLongPtrW(hwnd_h, GCLP_HICON) };
    if class_icon != 0 {
        return Some(HICON(class_icon as *mut _));
    }

    None
}

/// custom: sends a `SocketMessage` to the komorebi command socket
fn send_socket_message(message: SocketMessage) {
    std::thread::spawn(move || {
        if let Ok(json) = serde_json::to_string(&message)
            && let Ok(mut stream) = UnixStream::connect(DATA_DIR.join("komorebi.sock"))
        {
            let _ = stream.write_all(json.as_bytes());
        }
    });
}

impl From<isize> for Stackbar {
    fn from(value: isize) -> Self {
        Self { hwnd: value }
    }
}

impl Stackbar {
    pub const fn hwnd(&self) -> HWND {
        HWND(windows_api::as_ptr!(self.hwnd))
    }

    pub fn create(id: &str) -> color_eyre::Result<Self> {
        let name: Vec<u16> = format!("komostackbar-{id}\0").encode_utf16().collect();
        let class_name = PCWSTR(name.as_ptr());

        let h_module = WindowsApi::module_handle_w()?;

        let window_class = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(Self::callback),
            hInstance: h_module.into(),
            lpszClassName: class_name,
            hbrBackground: WindowsApi::create_solid_brush(0),
            ..Default::default()
        };

        let _ = WindowsApi::register_class_w(&window_class);

        let (hwnd_sender, hwnd_receiver) = mpsc::channel();

        let name_cl = name.clone();
        let instance = h_module.0 as isize;
        std::thread::spawn(move || -> color_eyre::Result<()> {
            unsafe {
                let hwnd = CreateWindowExW(
                    WS_EX_TOOLWINDOW | WS_EX_LAYERED,
                    PCWSTR(name_cl.as_ptr()),
                    PCWSTR(name_cl.as_ptr()),
                    WS_POPUP | WS_VISIBLE,
                    0,
                    0,
                    0,
                    0,
                    None,
                    None,
                    Option::from(HINSTANCE(windows_api::as_ptr!(instance))),
                    None,
                )?;

                SetLayeredWindowAttributes(hwnd, COLORREF(0), 0, LWA_COLORKEY)?;
                hwnd_sender.send(hwnd.0 as isize)?;

                let mut msg: MSG = MSG::default();

                loop {
                    if !GetMessageW(&mut msg, None, 0, 0).as_bool() {
                        tracing::debug!("stackbar window event processing thread shutdown");
                        break;
                    };
                    // TODO: error handling
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);

                    std::thread::sleep(Duration::from_millis(10))
                }
            }

            Ok(())
        });

        Ok(Self {
            hwnd: hwnd_receiver.recv()?,
        })
    }

    pub fn destroy(&self) -> color_eyre::Result<()> {
        WindowsApi::close_window(self.hwnd)
    }

    pub fn update(
        &self,
        container_padding: i32,
        container: &mut Container,
        layout: &Rect,
    ) -> color_eyre::Result<()> {
        let width = STACKBAR_TAB_WIDTH.load_consume();
        let height = STACKBAR_TAB_HEIGHT.load_consume();
        let gap = DEFAULT_CONTAINER_PADDING.load_consume();
        let background = STACKBAR_TAB_BACKGROUND_COLOUR.load_consume();
        let focused_text_colour = STACKBAR_FOCUSED_TEXT_COLOUR.load_consume();
        let unfocused_text_colour = STACKBAR_UNFOCUSED_TEXT_COLOUR.load_consume();

        let mut stackbars_containers = STACKBARS_CONTAINERS.lock();
        stackbars_containers.insert(self.hwnd, container.clone());

        let mut layout = *layout;
        let workspace_specific_offset =
            BORDER_WIDTH.load_consume() + BORDER_OFFSET.load_consume() + container_padding;

        layout.top -= workspace_specific_offset + STACKBAR_TAB_HEIGHT.load_consume();
        layout.left -= workspace_specific_offset;

        // Async causes the stackbar to disappear or flicker because we modify it right after,
        // so we have to do a synchronous call
        WindowsApi::position_window(self.hwnd, &layout, false, false)?;

        unsafe {
            let hdc = GetDC(Option::from(self.hwnd()));

            let hpen = CreatePen(PS_SOLID, 0, COLORREF(background));
            let hbrush = CreateSolidBrush(COLORREF(background));

            SelectObject(hdc, hpen.into());
            SelectObject(hdc, hbrush.into());
            SetBkColor(hdc, COLORREF(background));

            let mut logfont = LOGFONTW {
                lfWeight: FW_BOLD.0 as i32,
                lfQuality: FONT_QUALITY(PROOF_QUALITY.0),
                lfFaceName: [0; 32],
                ..Default::default()
            };

            if let Some(font_name) = &*STACKBAR_FONT_FAMILY.lock() {
                let font = wide_string(font_name);
                for (i, &c) in font.iter().enumerate() {
                    logfont.lfFaceName[i] = c;
                }
            }

            let logical_height = -MulDiv(
                STACKBAR_FONT_SIZE.load(Ordering::SeqCst),
                72,
                GetDeviceCaps(Option::from(hdc), LOGPIXELSY),
            );

            logfont.lfHeight = logical_height;

            let hfont = CreateFontIndirectW(&logfont);

            SelectObject(hdc, hfont.into());

            for (i, window) in container.windows().iter().enumerate() {
                if window.hwnd == container.focused_window().copied().unwrap_or_default().hwnd {
                    SetTextColor(hdc, COLORREF(focused_text_colour));
                } else {
                    SetTextColor(hdc, COLORREF(unfocused_text_colour));
                }

                let left = gap + (i as i32 * (width + gap));
                let mut rect = Rect {
                    top: 0,
                    left,
                    right: left + width,
                    bottom: height,
                };

                match STYLE.load() {
                    BorderStyle::System => {
                        if *WINDOWS_11 {
                            // TODO: error handling
                            let _ = RoundRect(
                                hdc,
                                rect.left,
                                rect.top,
                                rect.right,
                                rect.bottom,
                                20,
                                20,
                            );
                        } else {
                            // TODO: error handling
                            let _ = Rectangle(hdc, rect.left, rect.top, rect.right, rect.bottom);
                        }
                    }
                    BorderStyle::Rounded => {
                        // TODO: error handling
                        let _ =
                            RoundRect(hdc, rect.left, rect.top, rect.right, rect.bottom, 20, 20);
                    }
                    BorderStyle::Square => {
                        // TODO: error handling
                        let _ = Rectangle(hdc, rect.left, rect.top, rect.right, rect.bottom);
                    }
                }

                let label = match STACKBAR_LABEL.load() {
                    StackbarLabel::Process => {
                        let exe = window.exe()?;
                        exe.trim_end_matches(".exe").to_string()
                    }
                    StackbarLabel::Title => window.title()?,
                };

                // custom: draw the application icon at the left edge of the tab
                let icon_size = 16;
                if let Some(icon) = window_icon(window.hwnd) {
                    let _ = DrawIconEx(
                        hdc,
                        rect.left + 4,
                        (height - icon_size) / 2,
                        icon,
                        icon_size,
                        icon_size,
                        0,
                        None,
                        DI_NORMAL,
                    );
                }

                let mut tab_title: Vec<u16> = label.encode_utf16().collect();

                // custom: make room for the icon and left-align the text
                rect.left += icon_size + 8;
                rect.right -= 10;

                DrawTextW(
                    hdc,
                    &mut tab_title,
                    &mut rect.into(),
                    DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS,
                );
            }

            ReleaseDC(Option::from(self.hwnd()), hdc);
            // TODO: error handling
            let _ = DeleteObject(hpen.into());
            // TODO: error handling
            let _ = DeleteObject(hbrush.into());
            // TODO: error handling
            let _ = DeleteObject(hfont.into());
        }

        Ok(())
    }

    pub fn get_position_from_container_layout(&self, layout: &Rect) -> Rect {
        Rect {
            bottom: STACKBAR_TAB_HEIGHT.load_consume(),
            ..*layout
        }
    }

    unsafe extern "system" fn callback(
        hwnd: HWND,
        msg: u32,
        w_param: WPARAM,
        l_param: LPARAM,
    ) -> LRESULT {
        unsafe {
            match msg {
                WM_SETCURSOR => match LoadCursorW(None, IDC_ARROW) {
                    Ok(cursor) => {
                        SetCursor(Some(cursor));
                        LRESULT(0)
                    }
                    Err(error) => {
                        tracing::error!("{error}");
                        LRESULT(1)
                    }
                },
                WM_LBUTTONDOWN => {
                    let stackbars_containers = STACKBARS_CONTAINERS.lock();
                    if let Some(container) = stackbars_containers.get(&(hwnd.0 as isize)) {
                        let x = l_param.0 as i32 & 0xFFFF;

                        if let Some(index) = tab_index_at(container, x) {
                            focus_tab(container, index);
                        }
                    }

                    LRESULT(0)
                }
                WM_RBUTTONUP => {
                    let stackbars_containers = STACKBARS_CONTAINERS.lock();
                    if let Some(container) = stackbars_containers.get(&(hwnd.0 as isize)) {
                        let x = l_param.0 as i32 & 0xFFFF;
                        let y = (l_param.0 as i32 >> 16) & 0xFFFF;

                        if let Some(index) = tab_index_at(container, x) {
                            // Focus the tab that was right-clicked, then offer actions for it
                            focus_tab(container, index);
                            let window_hwnd = container
                                .windows()
                                .get(index)
                                .map(|w| w.hwnd)
                                .unwrap_or_default();

                            let mut point = POINT { x, y };
                            let _ = ClientToScreen(hwnd, &mut point);

                            match CreatePopupMenu() {
                                Ok(menu) => {
                                    let _ = AppendMenuW(
                                        menu,
                                        MF_STRING,
                                        1,
                                        PCWSTR(wide_string("Fechar").as_ptr()),
                                    );
                                    let _ = AppendMenuW(
                                        menu,
                                        MF_STRING,
                                        2,
                                        PCWSTR(wide_string("Remover da stack").as_ptr()),
                                    );
                                    let _ = AppendMenuW(
                                        menu,
                                        MF_STRING,
                                        3,
                                        PCWSTR(wide_string("Mover para o outro monitor").as_ptr()),
                                    );

                                    // With TPM_RETURNCMD the menu id is returned directly in the BOOL
                                    let choice = TrackPopupMenuEx(
                                        menu,
                                        (TPM_RIGHTBUTTON | TPM_RETURNCMD | TPM_NONOTIFY).0,
                                        point.x,
                                        point.y,
                                        hwnd,
                                        None,
                                    )
                                    .0;

                                    let _ = DestroyMenu(menu);

                                    match choice {
                                        1 => {
                                            if let Err(err) = WindowsApi::close_window(window_hwnd) {
                                                tracing::error!(
                                                    "stackbar context menu close error: {err}"
                                                );
                                            }
                                        }
                                        2 => {
                                            // Wait for the focus change events triggered by
                                            // focus_tab to process before unstacking the window
                                            std::thread::spawn(move || {
                                                std::thread::sleep(Duration::from_millis(300));
                                                send_socket_message(SocketMessage::UnstackWindow);
                                            });
                                        }
                                        3 => {
                                            // The click on the menu focuses the tab; wait for the
                                            // focus events to process, then send the focused window
                                            // alone to the next monitor
                                            std::thread::spawn(move || {
                                                std::thread::sleep(Duration::from_millis(300));
                                                send_socket_message(
                                                    SocketMessage::SendFocusedWindowToNextMonitor,
                                                );
                                            });
                                        }
                                        _ => {}
                                    }
                                }
                                Err(error) => {
                                    tracing::error!("stackbar context menu error: {error}");
                                }
                            }
                        }
                    }

                    LRESULT(0)
                }
                WM_DESTROY => {
                    PostQuitMessage(0);
                    LRESULT(0)
                }
                _ => DefWindowProcW(hwnd, msg, w_param, l_param),
            }
        }
    }
}

fn wide_string(s: &str) -> Vec<u16> {
    std::ffi::OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}
