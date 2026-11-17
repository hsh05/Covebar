//! App icons and display names via the Windows shell, encoded as PNG data URLs.

use crate::win::wide;
use base64::Engine;
use std::collections::HashMap;
use std::sync::Mutex;
use windows::core::PCWSTR;
use windows::Win32::Foundation::SIZE;
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, BITMAP, BITMAPINFO,
    BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP, HGDIOBJ,
};
use windows::Win32::System::Com::{CoInitializeEx, CoTaskMemFree, COINIT_APARTMENTTHREADED};
use windows::Win32::UI::Shell::{
    IShellItem, IShellItemImageFactory, SHCreateItemFromParsingName, SIGDN_NORMALDISPLAY,
    SIIGBF_ICONONLY,
};

static ICON_CACHE: Mutex<Option<HashMap<String, Option<String>>>> = Mutex::new(None);

pub fn com_init() {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }
}

/// Turn one part of an icon key into a shell parsing name.
/// Keys look like `aumid:<AppUserModelID>` or `path:<file path>`, joined by `|` as fallbacks.
fn parsing_name(part: &str) -> String {
    if let Some(a) = part.strip_prefix("aumid:") {
        format!("shell:AppsFolder\\{a}")
    } else {
        part.strip_prefix("path:").unwrap_or(part).to_string()
    }
}

/// Icon as a `data:image/png;base64,...` URL, cached.
pub fn icon_data_url(key: &str, size: i32) -> Option<String> {
    let cache_key = format!("{size}|{key}");
    if let Some(hit) = ICON_CACHE
        .lock()
        .unwrap()
        .get_or_insert_with(HashMap::new)
        .get(&cache_key)
    {
        return hit.clone();
    }
    com_init();
    let url = key.split('|').find_map(|part| shell_icon_png(&parsing_name(part), size)).map(|png| {
        format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(png)
        )
    });
    ICON_CACHE
        .lock()
        .unwrap()
        .get_or_insert_with(HashMap::new)
        .insert(cache_key, url.clone());
    url
}

/// Friendly display name for a shell item (e.g. `shell:AppsFolder\<aumid>`).
pub fn display_name(parsing: &str) -> Option<String> {
    com_init();
    let w = wide(parsing);
    unsafe {
        let item: IShellItem = SHCreateItemFromParsingName(PCWSTR(w.as_ptr()), None).ok()?;
        let p = item.GetDisplayName(SIGDN_NORMALDISPLAY).ok()?;
        let s = p.to_string().ok();
        CoTaskMemFree(Some(p.0 as *const _));
        s.filter(|s| !s.is_empty())
    }
}

fn shell_icon_png(parsing: &str, size: i32) -> Option<Vec<u8>> {
    let w = wide(parsing);
    unsafe {
        let factory: IShellItemImageFactory =
            SHCreateItemFromParsingName(PCWSTR(w.as_ptr()), None).ok()?;
        let hbmp = factory.GetImage(SIZE { cx: size, cy: size }, SIIGBF_ICONONLY).ok()?;
        let rgba = hbitmap_rgba(hbmp);
        let _ = DeleteObject(HGDIOBJ(hbmp.0));
        let (w, h, px) = rgba?;
        let mut out = Vec::new();
        image::codecs::png::PngEncoder::new(&mut out)
            .write_image(&px, w, h, image::ExtendedColorType::Rgba8)
            .ok()?;
        Some(out)
    }
}

use image::ImageEncoder;

unsafe fn hbitmap_rgba(hbmp: HBITMAP) -> Option<(u32, u32, Vec<u8>)> {
    let mut bm = BITMAP::default();
    if GetObjectW(
        HGDIOBJ(hbmp.0),
        std::mem::size_of::<BITMAP>() as i32,
        Some(&mut bm as *mut _ as *mut _),
    ) == 0
    {
        return None;
    }
    let (w, h) = (bm.bmWidth, bm.bmHeight.abs());
    if w <= 0 || h <= 0 {
        return None;
    }
    let mut bi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w,
            biHeight: -h,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut buf = vec![0u8; (w * h * 4) as usize];
    let dc = CreateCompatibleDC(None);
    let lines = GetDIBits(
        dc,
        hbmp,
        0,
        h as u32,
        Some(buf.as_mut_ptr() as *mut _),
        &mut bi,
        DIB_RGB_COLORS,
    );
    let _ = DeleteDC(dc);
    if lines == 0 {
        return None;
    }
    let has_alpha = buf.chunks_exact(4).any(|p| p[3] != 0);
    for p in buf.chunks_exact_mut(4) {
        let (b, g, r, a) = (p[0], p[1], p[2], p[3]);
        if !has_alpha {
            p.copy_from_slice(&[r, g, b, 255]);
        } else if a == 0 {
            p.copy_from_slice(&[0, 0, 0, 0]);
        } else {
            // Shell bitmaps are premultiplied; convert to straight alpha.
            let un = |c: u8| ((c as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8;
            p.copy_from_slice(&[un(r), un(g), un(b), a]);
        }
    }
    Some((w as u32, h as u32, buf))
}
