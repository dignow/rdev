use crate::rdev::{Button, EventType, RawKey, SimulateError};
use crate::windows::keycodes::get_win_codes;
use crate::Key;
use std::convert::TryFrom;
use std::mem::size_of;
use std::ptr::null_mut;
use winapi::ctypes::{c_int, c_short};
use winapi::shared::minwindef::{DWORD, LOWORD, UINT, WORD};
use winapi::shared::ntdef::LONG;
use winapi::um::winuser::{
    GetForegroundWindow, GetKeyboardLayout, GetSystemMetrics, GetWindowThreadProcessId, INPUT_u,
    MapVirtualKeyExW, SendInput, VkKeyScanW, INPUT, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT,
    KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE, KEYEVENTF_UNICODE,
    MAPVK_VSC_TO_VK_EX, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_HWHEEL, MOUSEEVENTF_LEFTDOWN,
    MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_MOVE,
    MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_VIRTUALDESK, MOUSEEVENTF_WHEEL,
    MOUSEEVENTF_XDOWN, MOUSEEVENTF_XUP, MOUSEINPUT, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
    WHEEL_DELTA,
};
/// Not defined in win32 but define here for clarity
static KEYEVENTF_KEYDOWN: DWORD = 0;
// KEYBDINPUT
static mut DW_MOUSE_EXTRA_INFO: usize = 0;
static mut DW_KEYBOARD_EXTRA_INFO: usize = 0;

#[derive(Debug, PartialEq, Eq)]
struct SimEvent {
    vkcode: WORD,
    modifier: Option<Key>,
}

impl SimEvent {
    pub fn new(vkcode: WORD) -> SimEvent {
        SimEvent {
            vkcode,
            modifier: None,
        }
    }

    pub fn with_modifier(vkcode: WORD, modifier: Key) -> SimEvent {
        SimEvent {
            vkcode,
            modifier: Some(modifier),
        }
    }
}

pub fn set_dw_mouse_extra_info(extra: usize) {
    unsafe { DW_MOUSE_EXTRA_INFO = extra }
}

pub fn set_dw_keyboard_extra_info(extra: usize) {
    unsafe { DW_KEYBOARD_EXTRA_INFO = extra }
}

fn sim_mouse_event(flags: DWORD, data: DWORD, dx: LONG, dy: LONG) -> Result<(), SimulateError> {
    let mut union: INPUT_u = unsafe { std::mem::zeroed() };
    let inner_union = unsafe { union.mi_mut() };
    unsafe {
        *inner_union = MOUSEINPUT {
            dx,
            dy,
            mouseData: data,
            dwFlags: flags,
            time: 0,
            dwExtraInfo: DW_MOUSE_EXTRA_INFO,
        };
    }
    let mut input = [INPUT {
        type_: INPUT_MOUSE,
        u: union,
    }; 1];
    let value = unsafe {
        SendInput(
            input.len() as UINT,
            input.as_mut_ptr(),
            size_of::<INPUT>() as c_int,
        )
    };
    if value != 1 {
        Err(SimulateError)
    } else {
        Ok(())
    }
}

fn sim_keyboard_event(flags: DWORD, vk: WORD, scan: WORD) -> Result<(), SimulateError> {
    let mut union: INPUT_u = unsafe { std::mem::zeroed() };
    let inner_union = unsafe { union.ki_mut() };
    unsafe {
        *inner_union = KEYBDINPUT {
            wVk: vk,
            wScan: scan,
            dwFlags: flags,
            time: 0,
            dwExtraInfo: DW_KEYBOARD_EXTRA_INFO,
        };
    }
    let mut input = [INPUT {
        type_: INPUT_KEYBOARD,
        u: union,
    }; 1];
    let value = unsafe {
        SendInput(
            input.len() as UINT,
            input.as_mut_ptr(),
            size_of::<INPUT>() as c_int,
        )
    };
    if value != 1 {
        Err(SimulateError)
    } else {
        Ok(())
    }
}

pub fn simulate(event_type: &EventType) -> Result<(), SimulateError> {
    match event_type {
        EventType::KeyPress(key) => {
            match key {
                crate::Key::RawKey(raw_key) => match raw_key {
                    RawKey::ScanCode(scancode) => {
                        let flags = if (scancode >> 8) == 0xE0 || (scancode >> 8) == 0xE1 {
                            KEYEVENTF_EXTENDEDKEY | KEYEVENTF_SCANCODE
                        } else {
                            KEYEVENTF_SCANCODE
                        };
                        sim_keyboard_event(flags, 0, *scancode as _)
                    }
                    RawKey::WinVirtualKeycode(vk) => sim_keyboard_event(0, *vk as _, 0),
                    _ => Err(SimulateError),
                },
                _ => {
                    let layout = unsafe {
                        let current_window_thread_id =
                            GetWindowThreadProcessId(GetForegroundWindow(), null_mut());
                        GetKeyboardLayout(current_window_thread_id)
                    };
                    let (code, scancode) = get_win_codes(*key).ok_or(SimulateError)?;
                    let code = if code == 165 && LOWORD(layout as usize as u32) == 0x0412 {
                        winapi::um::winuser::VK_HANGUL as u32
                    } else if code == 165 {
                        // altgr
                        165
                    } else if scancode != 0 {
                        unsafe { MapVirtualKeyExW(scancode as _, MAPVK_VSC_TO_VK_EX, layout) }
                    } else {
                        code
                    };
                    sim_keyboard_event(KEYEVENTF_KEYDOWN, code as _, 0)
                }
            }
        }
        EventType::KeyRelease(key) => {
            match key {
                crate::Key::RawKey(raw_key) => match raw_key {
                    RawKey::ScanCode(scancode) => {
                        let flags = if (scancode >> 8) == 0xE0 || (scancode >> 8) == 0xE1 {
                            KEYEVENTF_KEYUP | KEYEVENTF_EXTENDEDKEY | KEYEVENTF_SCANCODE
                        } else {
                            KEYEVENTF_KEYUP | KEYEVENTF_SCANCODE
                        };
                        sim_keyboard_event(flags, 0, *scancode as _)
                    }
                    RawKey::WinVirtualKeycode(vk) => {
                        sim_keyboard_event(KEYEVENTF_KEYUP, *vk as _, 0)
                    }
                    _ => Err(SimulateError),
                },
                _ => {
                    let (code, scancode) = get_win_codes(*key).ok_or(SimulateError)?;
                    let code = if code == 165 {
                        // altgr
                        165
                    } else if scancode != 0 {
                        unsafe {
                            let current_window_thread_id =
                                GetWindowThreadProcessId(GetForegroundWindow(), null_mut());
                            let layout = GetKeyboardLayout(current_window_thread_id);
                            MapVirtualKeyExW(scancode as _, MAPVK_VSC_TO_VK_EX, layout)
                        }
                    } else {
                        code
                    };
                    sim_keyboard_event(KEYEVENTF_KEYUP, code as _, 0)
                }
            }
        }
        EventType::ButtonPress(button) => match button {
            Button::Left => sim_mouse_event(MOUSEEVENTF_LEFTDOWN, 0, 0, 0),
            Button::Middle => sim_mouse_event(MOUSEEVENTF_MIDDLEDOWN, 0, 0, 0),
            Button::Right => sim_mouse_event(MOUSEEVENTF_RIGHTDOWN, 0, 0, 0),
            Button::Unknown(code) => sim_mouse_event(MOUSEEVENTF_XDOWN, 0, 0, (*code).into()),
        },
        EventType::ButtonRelease(button) => match button {
            Button::Left => sim_mouse_event(MOUSEEVENTF_LEFTUP, 0, 0, 0),
            Button::Middle => sim_mouse_event(MOUSEEVENTF_MIDDLEUP, 0, 0, 0),
            Button::Right => sim_mouse_event(MOUSEEVENTF_RIGHTUP, 0, 0, 0),
            Button::Unknown(code) => sim_mouse_event(MOUSEEVENTF_XUP, 0, 0, (*code).into()),
        },
        EventType::Wheel { delta_x, delta_y } => {
            if *delta_x != 0 {
                sim_mouse_event(
                    MOUSEEVENTF_HWHEEL,
                    (c_short::try_from(*delta_x).map_err(|_| SimulateError)? * WHEEL_DELTA) as u32,
                    0,
                    0,
                )?;
            }

            if *delta_y != 0 {
                sim_mouse_event(
                    MOUSEEVENTF_WHEEL,
                    (c_short::try_from(*delta_y).map_err(|_| SimulateError)? * WHEEL_DELTA) as u32,
                    0,
                    0,
                )?;
            }
            Ok(())
        }
        EventType::MouseMove { x, y } => {
            let width = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
            let height = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };
            if width == 0 || height == 0 {
                return Err(SimulateError);
            }

            sim_mouse_event(
                MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
                0,
                (*x as i32 + 1) * 65535 / width,
                (*y as i32 + 1) * 65535 / height,
            )
        }
    }
}

pub fn simulate_code(
    vk: Option<u16>,
    scan: Option<u32>,
    pressed: bool,
) -> Result<(), SimulateError> {
    let keycode;
    let scancode;
    let mut flags;

    if let Some(scan) = scan {
        keycode = 0;
        scancode = scan;
        flags = KEYEVENTF_SCANCODE;
    } else if let Some(vk) = vk {
        keycode = vk;
        scancode = 0;
        flags = 0;
    } else {
        return Err(SimulateError);
    }

    if (scancode >> 8) == 0xE0 || (scancode >> 8) == 0xE1 {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }

    if !pressed {
        flags |= KEYEVENTF_KEYUP;
    }
    sim_keyboard_event(flags as _, keycode, scancode as _)
}

/// 1 Either SHIFT key is pressed.
/// 2 Either CTRL key is pressed.
/// 4 Either ALT key is pressed.
/// FIXME:
/// 8 The Hankaku key is pressed
/// 16 Reserved (defined by the keyboard layout driver).
/// 32 Reserved (defined by the keyboard layout driver).
/// refs: https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-vkkeyscanw
#[inline]
fn char_to_sim_event(chr: char) -> Option<SimEvent> {
    let res = unsafe { VkKeyScanW(chr as u16) };
    let vkcode = (res & 0xFF) as WORD;
    let flag = res >> 8;
    match flag {
        -1 => None,
        0 => Some(SimEvent::new(vkcode)),
        1 => Some(SimEvent::with_modifier(vkcode, Key::ShiftLeft)),
        2 => Some(SimEvent::with_modifier(vkcode, Key::ControlLeft)),
        4 => Some(SimEvent::with_modifier(vkcode, Key::Alt)),
        _ => None,
    }
}

fn simulate_vkcode(vkcode: WORD, press: bool) -> Result<(), SimulateError> {
    if press {
        sim_keyboard_event(KEYEVENTF_KEYDOWN, vkcode, 0)
    } else {
        sim_keyboard_event(KEYEVENTF_KEYUP, vkcode, 0)
    }
}

pub fn simulate_char(chr: char) -> Result<(), SimulateError> {
    // send char
    if let Some(sim_event) = char_to_sim_event(chr) {
        if let Some(modifier) = sim_event.modifier {
            simulate(&EventType::KeyPress(modifier))?;
            simulate_vkcode(sim_event.vkcode, true)?;
            simulate_vkcode(sim_event.vkcode, true)?;
            simulate(&EventType::KeyRelease(modifier))?;
            Ok(())
        } else {
            simulate_vkcode(sim_event.vkcode, true)?;
            simulate_vkcode(sim_event.vkcode, true)?;
            Ok(())
        }
    } else {
        simulate_unicode(chr as u16)
    }
}

pub fn simulate_unicode(unicode: u16) -> Result<(), SimulateError> {
    sim_keyboard_event(KEYEVENTF_UNICODE, 0, unicode)?;
    sim_keyboard_event(KEYEVENTF_UNICODE | KEYEVENTF_KEYUP, 0, unicode)
}

#[inline]
pub fn simulate_unistr(unistr: &str) -> Result<(), SimulateError> {
    for unicode in unistr.encode_utf16() {
        simulate_unicode(unicode)?;
    }
    Ok(())
}

mod test {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_convert_char() {
        // If the function succeeds, the low-order byte of the return value contains the
        // virtual-key code and the high-order byte contains the shift state,
        // which can be a combination of the following flag bits.

        // In US keyboard.
        let chr = 'a';
        let sim_event = char_to_sim_event(chr);
        assert_eq!(
            sim_event,
            Some(SimEvent {
                vkcode: 65,
                modifier: None,
            },)
        );

        let chr = 'A';
        let sim_event = char_to_sim_event(chr);
        assert_eq!(
            sim_event,
            Some(SimEvent {
                vkcode: 65,
                modifier: Some(Key::ShiftLeft),
            },)
        );

        let chr = '&';
        let sim_event = char_to_sim_event(chr);
        assert_eq!(
            sim_event,
            Some(SimEvent {
                vkcode: 55,
                modifier: Some(Key::ShiftLeft),
            },)
        );

        let chr = 'é';
        let sim_event = char_to_sim_event(chr);
        assert_eq!(sim_event, None);
    }
}
