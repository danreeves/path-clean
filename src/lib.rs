//! `path-clean` is a Rust port of the the `cleanname` procedure from the Plan 9 C library, and is similar to
//! [`path.Clean`](https://golang.org/pkg/path/#Clean) from the Go standard library. It works as follows:
//!
//! 1. Reduce multiple slashes to a single slash.
//! 2. Eliminate `.` path name elements (the current directory).
//! 3. Eliminate `..` path name elements (the parent directory) and the non-`.` non-`..`, element that precedes them.
//! 4. Eliminate `..` elements that begin a rooted path, that is, replace `/..` by `/` at the beginning of a path.
//! 5. Leave intact `..` elements that begin a non-rooted path.
//!
//! If the result of this process is an empty string, return the string `"."`, representing the current directory.
//!
//! It performs this transform lexically, without touching the filesystem. Therefore it doesn't do
//! any symlink resolution or absolute path resolution. For more information you can see ["Getting Dot-Dot
//! Right"](https://9p.io/sys/doc/lexnames.html).
//!
//! For convenience, the [`PathClean`] trait is exposed and comes implemented for [`std::path::{Path, PathBuf}`].
//!
//! ```rust
//! use std::path::PathBuf;
//! use path_clean::{clean, PathClean};
//! assert_eq!(clean("hello/world/.."), PathBuf::from("hello"));
//! assert_eq!(
//!     PathBuf::from("/test/../path/").clean(),
//!     PathBuf::from("/path")
//! );
//! ```
#![forbid(unsafe_code)]

use std::path::{Component, Path, PathBuf};

/// The Clean trait implements a `clean` method.
pub trait PathClean {
    fn clean(&self) -> PathBuf;
}

/// PathClean implemented for `Path`
impl PathClean for Path {
    fn clean(&self) -> PathBuf {
        clean(self)
    }
}

/// PathClean implemented for `PathBuf`
impl PathClean for PathBuf {
    fn clean(&self) -> PathBuf {
        clean(self)
    }
}

/// The core implementation. It performs the following, lexically:
/// 1. Reduce multiple slashes to a single slash.
/// 2. Eliminate `.` path name elements (the current directory).
/// 3. Eliminate `..` path name elements (the parent directory) and the non-`.` non-`..`, element that precedes them.
/// 4. Eliminate `..` elements that begin a rooted path, that is, replace `/..` by `/` at the beginning of a path.
/// 5. Leave intact `..` elements that begin a non-rooted path.
///
/// If the result of this process is an empty string, return the string `"."`, representing the current directory.
pub fn clean<P>(path: P) -> PathBuf
where
    P: AsRef<Path>,
{
    let mut out = Vec::new();

    for comp in path.as_ref().components() {
        match comp {
            Component::CurDir => (),
            Component::ParentDir => match out.last() {
                Some(Component::RootDir) => (),
                Some(Component::Normal(_)) => {
                    out.pop();
                }
                None
                | Some(Component::CurDir)
                | Some(Component::ParentDir)
                | Some(Component::Prefix(_)) => out.push(comp),
            },
            comp => out.push(comp),
        }
    }

    // Handle Windows absolute paths: ensure Prefix components are followed by RootDir when needed
    if cfg!(windows) && out.len() >= 2 {
        if let Some(Component::Prefix(_)) = out.first() {
            if let Some(Component::Normal(_)) = out.get(1) {
                // Insert RootDir after Prefix if the next component is Normal
                // This handles the case where cross-drive navigation results in [Prefix, Normal, ...]
                // but should be [Prefix, RootDir, Normal, ...] for absolute paths
                out.insert(1, Component::RootDir);
            }
        }
    }

    if !out.is_empty() {
        out.iter().collect()
    } else {
        PathBuf::from(".")
    }
}

#[cfg(test)]
mod tests {
    use super::{clean, PathClean};
    use std::path::{Path, PathBuf};

    #[test]
    fn test_empty_path_is_current_dir() {
        assert_eq!(clean(""), PathBuf::from("."));
    }

    #[test]
    fn test_clean_paths_dont_change() {
        let tests = vec![(".", "."), ("..", ".."), ("/", "/")];

        for test in tests {
            assert_eq!(clean(test.0), PathBuf::from(test.1));
        }
    }

    #[test]
    fn test_replace_multiple_slashes() {
        let tests = vec![
            ("/", "/"),
            ("//", "/"),
            ("///", "/"),
            (".//", "."),
            ("//..", "/"),
            ("..//", ".."),
            ("/..//", "/"),
            ("/.//./", "/"),
            ("././/./", "."),
            ("path//to///thing", "path/to/thing"),
            ("/path//to///thing", "/path/to/thing"),
        ];

        for test in tests {
            assert_eq!(clean(test.0), PathBuf::from(test.1));
        }
    }

    #[test]
    fn test_eliminate_current_dir() {
        let tests = vec![
            ("./", "."),
            ("/./", "/"),
            ("./test", "test"),
            ("./test/./path", "test/path"),
            ("/test/./path/", "/test/path"),
            ("test/path/.", "test/path"),
        ];

        for test in tests {
            assert_eq!(clean(test.0), PathBuf::from(test.1));
        }
    }

    #[test]
    fn test_eliminate_parent_dir() {
        let tests = vec![
            ("/..", "/"),
            ("/../test", "/test"),
            ("test/..", "."),
            ("test/path/..", "test"),
            ("test/../path", "path"),
            ("/test/../path", "/path"),
            ("test/path/../../", "."),
            ("test/path/../../..", ".."),
            ("/test/path/../../..", "/"),
            ("/test/path/../../../..", "/"),
            ("test/path/../../../..", "../.."),
            ("test/path/../../another/path", "another/path"),
            ("test/path/../../another/path/..", "another"),
            ("../test", "../test"),
            ("../test/", "../test"),
            ("../test/path", "../test/path"),
            ("../test/..", ".."),
        ];

        for test in tests {
            assert_eq!(clean(test.0), PathBuf::from(test.1));
        }
    }

    #[test]
    fn test_pathbuf_trait() {
        assert_eq!(
            PathBuf::from("/test/../path/").clean(),
            PathBuf::from("/path")
        );
    }

    #[test]
    fn test_path_trait() {
        assert_eq!(Path::new("/test/../path/").clean(), PathBuf::from("/path"));
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_windows_paths() {
        let tests = vec![
            ("\\..", "\\"),
            ("\\..\\test", "\\test"),
            ("test\\..", "."),
            ("test\\path\\..\\..\\..", ".."),
            ("test\\path/..\\../another\\path", "another\\path"), // Mixed
            ("test\\path\\my/path", "test\\path\\my\\path"),      // Mixed 2
            ("/dir\\../otherDir/test.json", "/otherDir/test.json"), // User example
            ("c:\\test\\..", "c:\\"),                             // issue #12
            ("c:/test/..", "c:/"),                                // issue #12
        ];

        for test in tests {
            assert_eq!(clean(test.0), PathBuf::from(test.1));
        }
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_windows_cross_driver_paths() {
        // Test cases for cross-driver relative paths - issue #16
        let tests = vec![
            // Simple cross-driver case
            ("D:\\test\\..\\..\\..\\..\\C:\\Users\\test", "C:\\Users\\test"),
            // Original reported case 
            ("D:\\a\\pnp-rs\\pnp-rs\\fixtures\\global-cache\\../../../../../../C:/Users/runneradmin/AppData/Local/Yarn/Berry/cache/source-map-npm-0.6.1-1a3621db16-10c0.zip/node_modules/source-map/", 
             "C:\\Users\\runneradmin\\AppData\\Local\\Yarn\\Berry\\cache\\source-map-npm-0.6.1-1a3621db16-10c0.zip\\node_modules\\source-map\\"),
            // Mixed slash case
            ("D:/test/../../../C:\\Users\\test", "C:\\Users\\test"),
        ];

        for test in tests {
            assert_eq!(clean(test.0), PathBuf::from(test.1));
        }
    }

    #[test]
    fn debug_cross_drive_issue() {
        // Test cases that should reveal the issue - using forward slashes for Unix compatibility
        let test_cases = vec![
            // Simple case
            ("C:/test/../Users", "C:/Users"),
            // Cross-drive case
            ("D:/test/../../C:/Users", "C:/Users"),
        ];
        
        for (input, expected) in test_cases {
            println!("Testing: {} -> expected: {}", input, expected);
            
            let path = Path::new(input);
            println!("Input components:");
            for (i, comp) in path.components().enumerate() {
                println!("  {}: {:?}", i, comp);
            }
            
            let result = clean(input);
            println!("Result: {:?}", result);
            println!("Result display: {}", result.display());
            
            let expected_path = PathBuf::from(expected);
            println!("Expected: {:?}", expected_path);
            println!("Expected display: {}", expected_path.display());
            
            println!("Match: {}", result == expected_path);
            println!("---");
            
            // Assert that it works correctly
            assert_eq!(result, expected_path, "Failed for input: {}", input);
        }
    }

    #[test]
    fn test_windows_prefix_root_fix() {
        // This test verifies that our fix doesn't break anything on Unix
        // and documents the expected behavior for Windows paths
        
        // These should work correctly regardless of the fix
        let basic_tests = vec![
            ("test/../other", "other"),
            ("./test", "test"),
            ("/test/../other", "/other"),
            ("", "."),
        ];
        
        for (input, expected) in basic_tests {
            assert_eq!(clean(input), PathBuf::from(expected));
        }
        
        // These are Windows-style paths that should work with forward slashes on Unix
        // and the fix should not affect them
        let windows_style_tests = vec![
            ("C:/test", "C:/test"),
            ("C:/test/../Users", "C:/Users"),
        ];
        
        for (input, expected) in windows_style_tests {
            assert_eq!(clean(input), PathBuf::from(expected));
        }
    }
}
