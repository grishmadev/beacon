pub mod types;
pub mod wifi;

pub fn mac_to_bytes(mac: &str) -> [u8; 6] {
    // mac in format 12:34:56:78:90:12
    let mac_arr = mac.split(":");
    let mut res = [0u8; 6];
    for (i, m) in mac_arr.enumerate() {
        res[i] = u8::from_str_radix(m, 16)
            .map_err(|e| format!("Invalid hex at index {}: {}", i * 2, e))
            .unwrap();
    }
    res
}
