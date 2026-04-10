use std::sync::Arc;

use rmcp::model::Tool;
use serde_json::json;

use crate::registry::{error_result, schema_from_json, text_result, ServiceRegistry, ToolHandler};

/// Register all location tools with the service registry.
pub fn register(registry: &mut ServiceRegistry) {
    registry.register(
        "location_get_current",
        Tool::new(
            "location_get_current",
            "Get the current geographic location of this Mac (latitude, longitude, and accuracy in meters). Requires Location Services to be enabled in System Settings.",
            schema_from_json(json!({
                "type": "object",
                "properties": {},
            })),
        ),
        handler_get_current(),
    );
}

fn handler_get_current() -> ToolHandler {
    Arc::new(|_args| {
        Box::pin(async move {
            // Use a Swift snippet executed via `do shell script` in AppleScript.
            // CoreLocation requires a run loop to deliver delegate callbacks.
            // We compile and run a small Swift program that requests a single
            // location update and prints "lat,lng,accuracy" on success.
            let swift_code = r#"
import CoreLocation
import Foundation

class Delegate: NSObject, CLLocationManagerDelegate {
    var done = false
    var location: CLLocation?

    func locationManager(_ manager: CLLocationManager, didUpdateLocations locations: [CLLocation]) {
        location = locations.last
        done = true
    }

    func locationManager(_ manager: CLLocationManager, didFailWithError error: Error) {
        done = true
    }
}

let delegate = Delegate()
let manager = CLLocationManager()
manager.delegate = delegate
manager.requestLocation()
RunLoop.current.run(until: Date(timeIntervalSinceNow: 10))

if let loc = delegate.location {
    print("\(loc.coordinate.latitude),\(loc.coordinate.longitude),\(loc.horizontalAccuracy)")
} else {
    print("ERROR: Could not get location. Ensure Location Services are enabled in System Settings > Privacy & Security > Location Services.")
}
"#;

            // Escape the Swift code for embedding in an AppleScript `do shell script`
            let escaped_swift = swift_code
                .replace('\\', "\\\\")
                .replace('"', "\\\"");

            let script = format!(
                r#"do shell script "/usr/bin/swift -e \"{}\"" "#,
                escaped_swift.replace('\n', "\\n")
            );

            match crate::macos::applescript::run_applescript(&script) {
                Ok(output) => {
                    let trimmed = output.trim();
                    if trimmed.starts_with("ERROR:") {
                        Ok(error_result(trimmed.to_string()))
                    } else {
                        let parts: Vec<&str> = trimmed.split(',').collect();
                        if parts.len() == 3 {
                            let result = json!({
                                "latitude": parts[0].trim().parse::<f64>().unwrap_or(0.0),
                                "longitude": parts[1].trim().parse::<f64>().unwrap_or(0.0),
                                "accuracy_meters": parts[2].trim().parse::<f64>().unwrap_or(-1.0),
                            });
                            Ok(text_result(
                                serde_json::to_string_pretty(&result)
                                    .unwrap_or_else(|_| trimmed.to_string()),
                            ))
                        } else {
                            Ok(error_result(format!(
                                "Unexpected location output: {trimmed}"
                            )))
                        }
                    }
                }
                Err(e) => Ok(error_result(format!(
                    "Failed to get current location: {e}. \
                     CoreLocation may require a GUI application context or \
                     Location Services may be disabled."
                ))),
            }
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_schemas_valid() {
        let mut registry = ServiceRegistry::new();
        register(&mut registry);
        let tools = registry.list_tools();
        assert_eq!(tools.len(), 1, "Expected exactly 1 location tool");

        let names: Vec<_> = tools.iter().map(|t| t.name.as_ref()).collect();
        assert!(names.contains(&"location_get_current"));
    }
}
