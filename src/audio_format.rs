use std::{fmt, path::Path, str::FromStr};

use anyhow::{Result, bail};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioFormat {
    Flac,
    Aiff,
    Wav,
}

impl AudioFormat {
    pub fn accepted_values() -> &'static str {
        "flac, aiff, wav"
    }

    pub fn canonical_extension(self) -> &'static str {
        match self {
            Self::Flac => "flac",
            Self::Aiff => "aiff",
            Self::Wav => "wav",
        }
    }

    pub fn from_path(path: &Path) -> Option<Self> {
        path.extension()
            .and_then(|extension| extension.to_str())
            .and_then(Self::from_extension)
    }

    pub fn from_extension(extension: &str) -> Option<Self> {
        if extension.eq_ignore_ascii_case("flac") {
            Some(Self::Flac)
        } else if extension.eq_ignore_ascii_case("aiff") || extension.eq_ignore_ascii_case("aif") {
            Some(Self::Aiff)
        } else if extension.eq_ignore_ascii_case("wav") {
            Some(Self::Wav)
        } else {
            None
        }
    }
}

impl FromStr for AudioFormat {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        if value.eq_ignore_ascii_case("flac") {
            Ok(Self::Flac)
        } else if value.eq_ignore_ascii_case("aiff") {
            Ok(Self::Aiff)
        } else if value.eq_ignore_ascii_case("wav") {
            Ok(Self::Wav)
        } else {
            bail!(
                "unsupported audio format '{value}' (expected one of: {})",
                Self::accepted_values()
            )
        }
    }
}

impl fmt::Display for AudioFormat {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.canonical_extension())
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::AudioFormat;

    #[test]
    fn detects_formats_from_extensions() {
        assert_eq!(
            AudioFormat::from_path(Path::new("song.flac")),
            Some(AudioFormat::Flac)
        );
        assert_eq!(
            AudioFormat::from_path(Path::new("song.aiff")),
            Some(AudioFormat::Aiff)
        );
        assert_eq!(
            AudioFormat::from_path(Path::new("song.aif")),
            Some(AudioFormat::Aiff)
        );
        assert_eq!(
            AudioFormat::from_path(Path::new("song.wav")),
            Some(AudioFormat::Wav)
        );
        assert_eq!(AudioFormat::from_path(Path::new("song.txt")), None);
    }

    #[test]
    fn detects_extensions_case_insensitively() {
        assert_eq!(
            AudioFormat::from_path(Path::new("song.FLAC")),
            Some(AudioFormat::Flac)
        );
        assert_eq!(
            AudioFormat::from_path(Path::new("song.AiF")),
            Some(AudioFormat::Aiff)
        );
        assert_eq!(
            AudioFormat::from_path(Path::new("song.WAV")),
            Some(AudioFormat::Wav)
        );
    }

    #[test]
    fn exposes_canonical_extensions() {
        assert_eq!(AudioFormat::Flac.canonical_extension(), "flac");
        assert_eq!(AudioFormat::Aiff.canonical_extension(), "aiff");
        assert_eq!(AudioFormat::Wav.canonical_extension(), "wav");
    }

    #[test]
    fn displays_as_canonical_extension() {
        assert_eq!(AudioFormat::Flac.to_string(), "flac");
        assert_eq!(AudioFormat::Aiff.to_string(), "aiff");
        assert_eq!(AudioFormat::Wav.to_string(), "wav");
    }

    #[test]
    fn parses_supported_target_values() {
        assert_eq!("flac".parse::<AudioFormat>().unwrap(), AudioFormat::Flac);
        assert_eq!("AIFF".parse::<AudioFormat>().unwrap(), AudioFormat::Aiff);
        assert_eq!("Wav".parse::<AudioFormat>().unwrap(), AudioFormat::Wav);
    }

    #[test]
    fn rejects_aif_as_target_value() {
        let error = "aif".parse::<AudioFormat>().expect_err("parse should fail");
        assert!(error.to_string().contains("unsupported audio format"));
    }
}
