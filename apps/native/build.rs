fn main() {
    slint_build::compile("ui/main.slint").expect("Slint UI compilation failed");
    embed_windows_icon();
}

fn embed_windows_icon() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os != "windows" || target_env != "msvc" {
        return;
    }

    let manifest_dir = std::path::PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR missing"),
    );
    let ico_path = manifest_dir.join("assets").join("icon.ico");
    let ico = std::fs::read(&ico_path).expect("failed to read assets/icon.ico");
    let res = build_ico_resource(&ico).expect("invalid assets/icon.ico");

    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR missing"));
    let res_path = out_dir.join("druckwandel-icon.res");
    std::fs::write(&res_path, res).expect("failed to write Windows .res icon resource");
    println!(
        "cargo:rustc-link-arg-bin=druckwandel-native={}",
        res_path.display()
    );
}

/// Compile a Windows `.ico` file directly into a 32-bit Win32 `.res` resource file
/// containing `RT_ICON` entries (IDs 1..=N) and an `RT_GROUP_ICON` entry (ID 1).
fn build_ico_resource(ico: &[u8]) -> Result<Vec<u8>, &'static str> {
    if ico.len() < 6 {
        return Err("ICO header too short");
    }
    let reserved = u16::from_le_bytes([ico[0], ico[1]]);
    let ico_type = u16::from_le_bytes([ico[2], ico[3]]);
    let count = u16::from_le_bytes([ico[4], ico[5]]) as usize;
    if reserved != 0 || ico_type != 1 || count == 0 {
        return Err("unsupported ICO header");
    }
    let dir_end = 6 + count * 16;
    if ico.len() < dir_end {
        return Err("ICO directory truncated");
    }

    let mut out = Vec::with_capacity(ico.len() + 512);
    // Empty 32-bit .res file header marker (32 bytes).
    append_res_entry(&mut out, 0, 0, 0, 0, &[]);

    let mut group = Vec::with_capacity(6 + count * 14);
    group.extend_from_slice(&0u16.to_le_bytes());
    group.extend_from_slice(&1u16.to_le_bytes());
    group.extend_from_slice(&(count as u16).to_le_bytes());

    for index in 0..count {
        let entry = &ico[6 + index * 16..6 + (index + 1) * 16];
        let size = u32::from_le_bytes([entry[8], entry[9], entry[10], entry[11]]) as usize;
        let offset = u32::from_le_bytes([entry[12], entry[13], entry[14], entry[15]]) as usize;
        let end = offset.checked_add(size).ok_or("ICO image overflow")?;
        if end > ico.len() {
            return Err("ICO image out of bounds");
        }
        let icon_id = u16::try_from(index + 1).map_err(|_| "too many ICO entries")?;
        // RT_ICON = 3
        append_res_entry(&mut out, 3, icon_id, 0x1030, 0, &ico[offset..end]);
        group.extend_from_slice(&entry[0..12]);
        group.extend_from_slice(&icon_id.to_le_bytes());
    }

    // RT_GROUP_ICON = 14, ID = 1 (primary application icon for Explorer/Taskbar)
    append_res_entry(&mut out, 14, 1, 0x1030, 0, &group);
    Ok(out)
}

fn append_res_entry(
    out: &mut Vec<u8>,
    type_id: u16,
    name_id: u16,
    memory_flags: u16,
    language_id: u16,
    data: &[u8],
) {
    let data_size = u32::try_from(data.len()).expect("resource too large");
    let header_size: u32 = 32;
    out.extend_from_slice(&data_size.to_le_bytes());
    out.extend_from_slice(&header_size.to_le_bytes());
    out.extend_from_slice(&0xFFFFu16.to_le_bytes());
    out.extend_from_slice(&type_id.to_le_bytes());
    out.extend_from_slice(&0xFFFFu16.to_le_bytes());
    out.extend_from_slice(&name_id.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // DataVersion
    out.extend_from_slice(&memory_flags.to_le_bytes());
    out.extend_from_slice(&language_id.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // Version
    out.extend_from_slice(&0u32.to_le_bytes()); // Characteristics
    out.extend_from_slice(data);
    while !out.len().is_multiple_of(4) {
        out.push(0);
    }
}
