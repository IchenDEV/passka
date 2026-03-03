import Foundation

/// Swift wrapper around the Rust FFI functions.
/// Calls the passka CLI as a subprocess for reliable cross-language interop.
/// This avoids swift-bridge build complexity while sharing the same Keychain data.
public enum PasskaBridge {

    private static let cliPath: String = {
        let home = FileManager.default.homeDirectoryForCurrentUser.path
        let candidates = [
            "\(home)/.cargo/bin/passka",
            "/usr/local/bin/passka",
            "/opt/homebrew/bin/passka",
        ]
        return candidates.first { FileManager.default.fileExists(atPath: $0) }
            ?? "passka"
    }()

    public static func listCredentials(typeFilter: String? = nil) -> [[String: Any]] {
        var args = ["list", "--type", "json"]
        if let t = typeFilter { args = ["list", "--type", t] }
        // Fall back to reading the index file directly for the GUI
        return readIndex(typeFilter: typeFilter)
    }

    public static func getCredentialMeta(name: String) -> [String: Any]? {
        let entries = readIndex()
        return entries.first { ($0["name"] as? String) == name }
    }

    public static func getCredentialValue(name: String, field: String) -> String? {
        let result = shell([cliPath, "get", name, "--field", field])
        return result.isEmpty ? nil : result
    }

    public static func addCredentialRaw(
        name: String, type: String, fields: [String: String], description: String
    ) -> Bool {
        guard let jsonData = try? JSONSerialization.data(withJSONObject: ["fields": fields]),
              let json = String(data: jsonData, encoding: .utf8) else { return false }
        // Use the core library's index file + keychain directly
        // For now, we'll use the CLI approach
        let fieldsArgs = fields.flatMap { ["-f", "\($0.key)=\($0.value)"] }
        let _ = shell([cliPath, "add", name, "--type", type] + fieldsArgs)
        return true
    }

    public static func removeCredential(name: String) -> Bool {
        let _ = shell([cliPath, "rm", "--yes", name])
        return true
    }

    // MARK: - Private

    private static func readIndex(typeFilter: String? = nil) -> [[String: Any]] {
        let configDir = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".config/passka/index.json")
        guard let data = try? Data(contentsOf: configDir),
              let arr = try? JSONSerialization.jsonObject(with: data) as? [[String: Any]]
        else { return [] }
        if let filter = typeFilter {
            return arr.filter { ($0["cred_type"] as? String) == filter }
        }
        return arr
    }

    private static func shell(_ args: [String]) -> String {
        let proc = Process()
        let pipe = Pipe()
        proc.executableURL = URL(fileURLWithPath: args[0])
        proc.arguments = Array(args.dropFirst())
        proc.standardOutput = pipe
        proc.standardError = FileHandle.nullDevice
        try? proc.run()
        proc.waitUntilExit()
        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        return String(data: data, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
    }
}
