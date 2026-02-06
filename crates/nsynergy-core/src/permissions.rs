use serde::{Deserialize, Serialize};

/// Represents the status of an OS-level permission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionStatus {
    /// Permission has been granted.
    Granted,
    /// Permission has not been granted (or cannot be determined).
    Denied,
    /// Permission is not applicable on this platform.
    NotApplicable,
}

/// Summary of all required permissions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionCheck {
    pub accessibility: PermissionStatus,
    pub input_monitoring: PermissionStatus,
}

/// Checks whether the required OS permissions are granted.
///
/// On macOS, this checks Accessibility and Input Monitoring permissions.
/// On other platforms, returns `NotApplicable` for all permissions.
pub fn check_permissions() -> PermissionCheck {
    #[cfg(target_os = "macos")]
    {
        check_macos_permissions()
    }
    #[cfg(not(target_os = "macos"))]
    {
        PermissionCheck {
            accessibility: PermissionStatus::NotApplicable,
            input_monitoring: PermissionStatus::NotApplicable,
        }
    }
}

#[cfg(target_os = "macos")]
fn check_macos_permissions() -> PermissionCheck {
    // On macOS, we use the ApplicationServices framework to check
    // if the app is trusted for accessibility access.
    // AXIsProcessTrusted() returns true if Accessibility is granted.
    let accessibility = if macos_ax_is_trusted() {
        PermissionStatus::Granted
    } else {
        PermissionStatus::Denied
    };

    // Input Monitoring uses the same AXIsProcessTrusted check in practice,
    // as both are required for global event capture via rdev.
    // A more granular check would use IOHIDCheckAccess, but that's private API.
    let input_monitoring = accessibility;

    PermissionCheck {
        accessibility,
        input_monitoring,
    }
}

#[cfg(target_os = "macos")]
fn macos_ax_is_trusted() -> bool {
    // AXIsProcessTrusted is in the ApplicationServices framework.
    // We link dynamically to avoid a hard dependency.
    unsafe extern "C" {
        fn AXIsProcessTrusted() -> bool;
    }
    // SAFETY: AXIsProcessTrusted is a well-known stable macOS API
    // that takes no arguments and returns a boolean.
    unsafe { AXIsProcessTrusted() }
}

/// Returns a user-friendly description of how to grant a permission.
pub fn permission_instructions(check: &PermissionCheck) -> Vec<String> {
    let mut instructions = Vec::new();

    #[cfg(target_os = "macos")]
    {
        if check.accessibility == PermissionStatus::Denied {
            instructions.push(
                "Accessibility: System Settings > Privacy & Security > Accessibility > Enable nsynergy"
                    .to_string(),
            );
        }
        if check.input_monitoring == PermissionStatus::Denied {
            instructions.push(
                "Input Monitoring: System Settings > Privacy & Security > Input Monitoring > Enable nsynergy"
                    .to_string(),
            );
        }
    }

    #[cfg(target_os = "windows")]
    {
        let _ = check; // suppress unused warning
        instructions.push(
            "Windows Firewall: Allow nsynergy through Windows Defender Firewall for private networks"
                .to_string(),
        );
    }

    instructions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_permissions_returns_valid_result() {
        let check = check_permissions();
        // On macOS CI, permissions are typically denied.
        // On other platforms, they should be NotApplicable.
        #[cfg(not(target_os = "macos"))]
        {
            assert_eq!(check.accessibility, PermissionStatus::NotApplicable);
            assert_eq!(check.input_monitoring, PermissionStatus::NotApplicable);
        }
        #[cfg(target_os = "macos")]
        {
            // Just check it doesn't panic; actual value depends on CI environment
            let _ = check.accessibility;
            let _ = check.input_monitoring;
        }
    }

    #[test]
    fn permission_instructions_for_denied() {
        let check = PermissionCheck {
            accessibility: PermissionStatus::Denied,
            input_monitoring: PermissionStatus::Denied,
        };
        let instructions = permission_instructions(&check);
        #[cfg(target_os = "macos")]
        {
            assert!(instructions.len() >= 1);
            assert!(instructions[0].contains("Accessibility"));
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            assert!(instructions.is_empty());
        }
    }

    #[test]
    fn permission_instructions_for_granted() {
        let check = PermissionCheck {
            accessibility: PermissionStatus::Granted,
            input_monitoring: PermissionStatus::Granted,
        };
        let instructions = permission_instructions(&check);
        // No instructions needed when all granted (on macOS)
        #[cfg(target_os = "macos")]
        assert!(instructions.is_empty());
    }

    #[test]
    fn permission_status_serialization() {
        let check = PermissionCheck {
            accessibility: PermissionStatus::Granted,
            input_monitoring: PermissionStatus::Denied,
        };
        let json = serde_json::to_string(&check).unwrap();
        let deserialized: PermissionCheck = serde_json::from_str(&json).unwrap();
        assert_eq!(check, deserialized);
    }
}
