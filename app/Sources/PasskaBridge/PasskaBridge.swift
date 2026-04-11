import Foundation

public enum PasskaBridge {
    private static let cliPath: String = {
        let home = FileManager.default.homeDirectoryForCurrentUser.path
        let candidates = [
            "\(home)/.cargo/bin/passka",
            "/usr/local/bin/passka",
            "/opt/homebrew/bin/passka",
            "\(home)/projects/passka/target/debug/passka",
        ]
        return candidates.first { FileManager.default.fileExists(atPath: $0) } ?? "passka"
    }()

    public static func listAccounts() -> [[String: Any]] {
        decodeArray(shell([cliPath, "account", "list"]))
    }

    public static func getAccount(accountId: String) -> [String: Any]? {
        decodeObject(shell([cliPath, "account", "show", accountId]))
    }

    public static func revealAccountField(
        actorPrincipalId: String = "principal:local-human",
        accountId: String,
        field: String
    ) -> String? {
        let value = shell([
            cliPath,
            "account",
            "reveal",
            accountId,
            "--field",
            field,
            "--principal",
            actorPrincipalId,
            "--raw",
        ])
        return value.isEmpty ? nil : value
    }

    public static func registerAPIKeyAccount(
        name: String,
        provider: String,
        baseURL: String,
        description: String,
        apiKey: String,
        headerName: String,
        headerPrefix: String
    ) -> Bool {
        let args = [
            cliPath,
            "account",
            "add",
            name,
            "--provider",
            provider,
            "--auth",
            "api_key",
            "--base-url",
            baseURL,
            "--description",
            description,
        ]
        return runInteractive(
            args,
            input: "\(apiKey)\n\(headerName)\n\(headerPrefix)\n\n"
        )
    }

    public static func registerOAuthAccount(
        name: String,
        provider: String,
        baseURL: String,
        description: String,
        authorizeURL: String,
        tokenURL: String,
        clientID: String,
        clientSecret: String,
        redirectURI: String,
        scopes: String
    ) -> Bool {
        let args = [
            cliPath,
            "account",
            "add",
            name,
            "--provider",
            provider,
            "--auth",
            "oauth",
            "--base-url",
            baseURL,
            "--description",
            description,
        ]
        return runInteractive(
            args,
            input: "\(authorizeURL)\n\(tokenURL)\n\(clientID)\n\(clientSecret)\n\(redirectURI)\n\(scopes)\n"
        )
    }

    public static func registerOpaqueAccount(
        name: String,
        provider: String,
        baseURL: String,
        description: String,
        fields: [String: String]
    ) -> Bool {
        var input = ""
        for key in fields.keys.sorted() {
            input += "\(key)\n\(fields[key] ?? "")\n"
        }
        input += "\n"
        let args = [
            cliPath,
            "account",
            "add",
            name,
            "--provider",
            provider,
            "--auth",
            "opaque",
            "--base-url",
            baseURL,
            "--description",
            description,
        ]
        return runInteractive(args, input: input)
    }

    public static func registerOTPAccount(
        name: String,
        provider: String,
        baseURL: String,
        description: String,
        seed: String,
        issuer: String,
        accountName: String,
        digits: String,
        period: String
    ) -> Bool {
        let args = [
            cliPath,
            "account",
            "add",
            name,
            "--provider",
            provider,
            "--auth",
            "otp",
            "--base-url",
            baseURL,
            "--description",
            description,
        ]
        return runInteractive(
            args,
            input: "\(seed)\n\(issuer)\n\(accountName)\n\(digits)\n\(period)\n"
        )
    }

    public static func removeAccount(accountId: String) -> Bool {
        runStatus([cliPath, "account", "remove", accountId]) == 0
    }

    public static func listPrincipals() -> [[String: Any]] {
        decodeArray(shell([cliPath, "principal", "list"]))
    }

    public static func addPrincipal(name: String, kind: String, description: String) -> Bool {
        runStatus([
            cliPath,
            "principal",
            "add",
            name,
            "--kind",
            kind,
            "--description",
            description,
        ]) == 0
    }

    public static func listPolicies() -> [[String: Any]] {
        decodeArray(shell([cliPath, "policy", "list"]))
    }

    public static func listAuditEvents(limit: Int? = nil) -> [[String: Any]] {
        var args = [cliPath, "audit", "list"]
        if let limit {
            args += ["--limit", "\(limit)"]
        }
        return decodeArray(shell(args))
    }

    private static func decodeArray(_ raw: String) -> [[String: Any]] {
        guard let data = raw.data(using: .utf8),
              let arr = try? JSONSerialization.jsonObject(with: data) as? [[String: Any]]
        else { return [] }
        return arr
    }

    private static func decodeObject(_ raw: String) -> [String: Any]? {
        guard let data = raw.data(using: .utf8),
              let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
        else { return nil }
        return object
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
        return String(data: data, encoding: .utf8)?
            .trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
    }

    private static func runStatus(_ args: [String]) -> Int32 {
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: args[0])
        proc.arguments = Array(args.dropFirst())
        proc.standardOutput = FileHandle.nullDevice
        proc.standardError = FileHandle.nullDevice
        try? proc.run()
        proc.waitUntilExit()
        return proc.terminationStatus
    }

    private static func runInteractive(_ args: [String], input: String) -> Bool {
        let proc = Process()
        let stdin = Pipe()
        proc.executableURL = URL(fileURLWithPath: args[0])
        proc.arguments = Array(args.dropFirst())
        proc.standardInput = stdin
        proc.standardOutput = FileHandle.nullDevice
        proc.standardError = FileHandle.nullDevice
        do {
            try proc.run()
            stdin.fileHandleForWriting.write(Data(input.utf8))
            try? stdin.fileHandleForWriting.close()
            proc.waitUntilExit()
            return proc.terminationStatus == 0
        } catch {
            return false
        }
    }
}
