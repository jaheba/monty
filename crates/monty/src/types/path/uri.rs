//! File URIs for the sandbox's virtual POSIX paths.

use monty_types::ResourceTracker;

use super::Path;
use crate::exception_private::{ExcType, ExcTypeExt, RunResult};

const HEX: &[u8; 16] = b"0123456789ABCDEF";

/// Percent-encodes UTF-8 bytes, leaving only URI-unreserved characters and slashes literal.
pub(super) fn as_uri(path: &Path, tracker: &ResourceTracker) -> RunResult<String> {
    if !path.is_absolute() {
        return Err(ExcType::value_error("relative paths can't be expressed as file URIs"));
    }

    let text = path.as_str();
    // Encoding expands each input byte to at most three bytes.
    let mut uri = String::with_capacity(text.len() + 7);
    uri.push_str("file://");
    for (index, byte) in text.bytes().enumerate() {
        tracker.check_time_every(index)?;
        if byte.is_ascii_alphanumeric() || b"/-._~".contains(&byte) {
            uri.push(char::from(byte));
        } else {
            uri.push('%');
            uri.push(char::from(HEX[usize::from(byte >> 4)]));
            uri.push(char::from(HEX[usize::from(byte & 0xf)]));
        }
    }
    Ok(uri)
}
