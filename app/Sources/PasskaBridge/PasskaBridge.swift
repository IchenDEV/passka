import Foundation

public enum PasskaBridge {
    private static let brokerAddress = "127.0.0.1:8478"
    private static let brokerBaseURL = URL(string: "http://\(brokerAddress)")!
    private static var brokerProcess: Process?

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
        guard ensureBrokerDaemonRunning() else { return nil }
        let payload: [String: String] = [
            "actor_principal_id": actorPrincipalId,
            "field": field,
        ]
        guard let data = try? JSONSerialization.data(withJSONObject: payload),
              let response = httpRequest(
                path: "/app/accounts/\(accountId)/reveal",
                method: "POST",
                body: data
              )
        else {
            return nil
        }
        return decodeString(response)
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

    public static func listAuthorizations() -> [[String: Any]] {
        guard ensureBrokerDaemonRunning(),
              let data = httpRequest(path: "/authorizations", method: "GET", body: nil)
        else {
            return []
        }
        return decodeArray(data)
    }

    public static func authorizeAccount(
        accountId: String,
        agentPrincipalId: String,
        leaseSeconds: Int,
        environments: [String] = [],
        description: String = ""
    ) -> Bool {
        var args = [
            cliPath,
            "account",
            "allow",
            accountId,
            "--agent",
            agentPrincipalId,
            "--lease-seconds",
            "\(leaseSeconds)",
        ]
        if !environments.isEmpty {
            args += ["--environments", environments.joined(separator: ",")]
        }
        if !description.isEmpty {
            args += ["--description", description]
        }
        return runStatus(args) == 0
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

    private static func decodeArray(_ data: Data) -> [[String: Any]] {
        guard let arr = try? JSONSerialization.jsonObject(with: data) as? [[String: Any]]
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

    private static func ensureBrokerDaemonRunning() -> Bool {
        if brokerHealthcheck() {
            return true
        }

        if brokerProcess?.isRunning != true {
            let proc = Process()
            proc.executableURL = URL(fileURLWithPath: cliPath)
            proc.arguments = ["broker", "serve", "--addr", brokerAddress]
            proc.standardOutput = FileHandle.nullDevice
            proc.standardError = FileHandle.nullDevice
            do {
                try proc.run()
                brokerProcess = proc
            } catch {
                return false
            }
        }

        for _ in 0..<20 {
            if brokerHealthcheck() {
                return true
            }
            Thread.sleep(forTimeInterval: 0.1)
        }
        return false
    }

    private static func brokerHealthcheck() -> Bool {
        guard let data = httpRequest(path: "/health", method: "GET", body: nil) else {
            return false
        }
        guard let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            return false
        }
        return object["ok"] as? Bool == true
    }

    private static func httpRequest(path: String, method: String, body: Data?) -> Data? {
        let url = brokerBaseURL.appending(path: path)
        var request = URLRequest(url: url)
        request.httpMethod = method
        request.timeoutInterval = 2
        if let body {
            request.httpBody = body
            request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        }

        let semaphore = DispatchSemaphore(value: 0)
        var responseData: Data?
        let task = URLSession.shared.dataTask(with: request) { data, response, _ in
            defer { semaphore.signal() }
            guard let http = response as? HTTPURLResponse,
                  (200..<300).contains(http.statusCode),
                  let data
            else {
                return
            }
            responseData = data
        }
        task.resume()
        _ = semaphore.wait(timeout: .now() + 3)
        return responseData
    }

    private static func decodeString(_ data: Data) -> String? {
        if let value = try? JSONSerialization.jsonObject(with: data) as? String {
            return value
        }
        if let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
           let error = object["error"] as? String {
            return error.isEmpty ? nil : nil
        }
        return String(data: data, encoding: .utf8)?
            .trimmingCharacters(in: .whitespacesAndNewlines)
    }
}
