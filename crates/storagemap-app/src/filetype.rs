//! File categories used for treemap colours and the legend.

use egui::Color32;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum Category {
    Image,
    Video,
    Audio,
    Archive,
    Document,
    Code,
    Executable,
    DiskImage,
    Other,
}

impl Category {
    pub const ALL: [Category; 9] = [
        Category::Video,
        Category::Image,
        Category::Audio,
        Category::Archive,
        Category::DiskImage,
        Category::Document,
        Category::Code,
        Category::Executable,
        Category::Other,
    ];

    pub fn of_extension(ext: &str) -> Category {
        match ext {
            "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "tif" | "tiff" | "heic" | "heif"
            | "avif" | "svg" | "ico" | "raw" | "cr2" | "cr3" | "nef" | "arw" | "dng" | "psd"
            | "xcf" => Category::Image,
            "mp4" | "mkv" | "avi" | "mov" | "wmv" | "webm" | "m4v" | "flv" | "mpg" | "mpeg"
            | "ts" | "m2ts" | "3gp" => Category::Video,
            "mp3" | "flac" | "wav" | "ogg" | "opus" | "m4a" | "aac" | "wma" | "aiff" | "alac"
            | "mid" => Category::Audio,
            "zip" | "7z" | "rar" | "tar" | "gz" | "tgz" | "bz2" | "xz" | "zst" | "lz4" | "cab"
            | "deb" | "rpm" | "msi" | "apk" | "jar" | "whl" | "snap" | "flatpak" | "appimage"
            | "pak" => Category::Archive,
            "pdf" | "doc" | "docx" | "odt" | "rtf" | "txt" | "md" | "xls" | "xlsx" | "ods"
            | "csv" | "ppt" | "pptx" | "odp" | "epub" | "html" | "htm" | "xml" | "json" | "log" => {
                Category::Document
            }
            "rs" | "c" | "h" | "cpp" | "hpp" | "cc" | "py" | "js" | "mjs" | "tsx" | "jsx"
            | "java" | "kt" | "go" | "rb" | "php" | "cs" | "swift" | "sh" | "ps1" | "lua"
            | "toml" | "yaml" | "yml" | "sql" | "o" | "obj" | "rlib" | "rmeta" | "pyc"
            | "class" | "pdb" => Category::Code,
            "exe" | "dll" | "so" | "dylib" | "sys" | "bin" | "elf" | "com" | "bat" | "cmd"
            | "efi" | "ko" | "a" | "lib" => Category::Executable,
            "iso" | "img" | "vhd" | "vhdx" | "vmdk" | "vdi" | "qcow2" | "dmg" | "wim" | "esd"
            | "swp" | "swap" => Category::DiskImage,
            _ => Category::Other,
        }
    }

    pub fn label_id(self) -> &'static str {
        match self {
            Category::Image => "cat-image",
            Category::Video => "cat-video",
            Category::Audio => "cat-audio",
            Category::Archive => "cat-archive",
            Category::Document => "cat-document",
            Category::Code => "cat-code",
            Category::Executable => "cat-executable",
            Category::DiskImage => "cat-disk-image",
            Category::Other => "cat-other",
        }
    }

    /// Tile colour; chosen to stay distinguishable in light and dark themes
    /// and for the common forms of colour blindness (Okabe–Ito based).
    pub fn color(self) -> Color32 {
        match self {
            Category::Video => Color32::from_rgb(213, 94, 0),
            Category::Image => Color32::from_rgb(204, 121, 167),
            Category::Audio => Color32::from_rgb(230, 159, 0),
            Category::Archive => Color32::from_rgb(160, 120, 70),
            Category::DiskImage => Color32::from_rgb(170, 70, 120),
            Category::Document => Color32::from_rgb(0, 114, 178),
            Category::Code => Color32::from_rgb(0, 158, 115),
            Category::Executable => Color32::from_rgb(86, 180, 233),
            Category::Other => Color32::from_rgb(140, 140, 150),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_common_extensions() {
        assert_eq!(Category::of_extension("mkv"), Category::Video);
        assert_eq!(Category::of_extension("qcow2"), Category::DiskImage);
        assert_eq!(Category::of_extension(""), Category::Other);
    }
}
