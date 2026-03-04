import Foundation
import Security

/// Swift wrapper around Passka storage.
/// Reads the index file and keychain directly for the GUI.
/// CLI is only used for write operations that need interactive input.
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

    private static let serviceName = "passka"

    public static func listCredentials(typeFilter: String? = nil) -> [[String: Any]] {
        readIndex(typeFilter: typeFilter)
    }

    public static func getCredentialMeta(name: String) -> [String: Any]? {
        let entries = readIndex()
        return entries.first { ($0["name"] as? String) == name }
    }

    /// Read a credential field directly from the macOS keychain.
    /// Requires Touch ID / authentication at the caller level.
    public static func getCredentialValue(name: String, field: String) -> String? {
        guard let data = readKeychainEntry(account: name) else { return nil }
        guard let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let fields = json["fields"] as? [String: String]
        else { return nil }
        return fields[field]
    }

    public static func addCredentialRaw(
        name: String, type: String, fields: [String: String], description: String
    ) -> Bool {
        guard let jsonData = try? JSONSerialization.data(withJSONObject: ["fields": fields]),
              let json = String(data: jsonData, encoding: .utf8) else { return false }
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

    private static func readKeychainEntry(account: String) -> Data? {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: serviceName,
            kSecAttrAccount as String: account,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne,
        ]
        var result: AnyObject?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        guard status == errSecSuccess else { return nil }
        return result as? Data
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
