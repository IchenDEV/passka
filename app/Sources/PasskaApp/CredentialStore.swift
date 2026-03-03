import Foundation
import Combine

struct CredentialEntry: Identifiable, Hashable {
    let id: String
    let name: String
    let credType: String
    let description: String
    let envVars: [String: String]
    let createdAt: String

    init(from dict: [String: Any]) {
        self.name = dict["name"] as? String ?? ""
        self.id = self.name
        self.credType = dict["cred_type"] as? String ?? "custom"
        self.description = dict["description"] as? String ?? ""
        self.envVars = dict["env_vars"] as? [String: String] ?? [:]
        self.createdAt = dict["created_at"] as? String ?? ""
    }
}

class CredentialStore: ObservableObject {
    @Published var entries: [CredentialEntry] = []
    @Published var selectedType: String? = nil

    private let indexURL: URL = {
        FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".config/passka/index.json")
    }()

    init() { reload() }

    func reload() {
        guard let data = try? Data(contentsOf: indexURL),
              let arr = try? JSONSerialization.jsonObject(with: data) as? [[String: Any]]
        else {
            entries = []
            return
        }
        entries = arr.map { CredentialEntry(from: $0) }
    }

    func filtered() -> [CredentialEntry] {
        guard let t = selectedType else { return entries }
        return entries.filter { $0.credType == t }
    }

    func remove(name: String) {
        let _ = PasskaBridge.removeCredential(name: name)
        reload()
    }

    func getValue(name: String, field: String) -> String? {
        PasskaBridge.getCredentialValue(name: name, field: field)
    }
}
