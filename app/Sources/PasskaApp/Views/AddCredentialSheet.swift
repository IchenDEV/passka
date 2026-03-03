import SwiftUI

struct AddCredentialSheet: View {
    @EnvironmentObject var store: CredentialStore
    @Environment(\.dismiss) private var dismiss

    @State private var name = ""
    @State private var credType = "api_key"
    @State private var description = ""
    @State private var fields: [FieldInput] = []
    @State private var errorMessage: String?

    private let typeOptions = [
        ("api_key", "API Key"),
        ("user_pass", "Username & Password"),
        ("cookie", "Cookie / Session"),
        ("app_secret", "App Secret (AK/SK)"),
        ("token", "OAuth Token"),
        ("custom", "Custom"),
    ]

    var body: some View {
        VStack(spacing: 0) {
            Form {
                Section("Basic Info") {
                    TextField("Name", text: $name)
                    Picker("Type", selection: $credType) {
                        ForEach(typeOptions, id: \.0) { id, label in
                            Text(label).tag(id)
                        }
                    }
                    TextField("Description", text: $description)
                }

                Section("Fields") {
                    ForEach($fields) { $field in
                        HStack {
                            Text(field.name).frame(width: 120, alignment: .leading)
                            if field.sensitive {
                                SecureField("Required", text: $field.value)
                            } else {
                                TextField(field.optional ? "Optional" : "Required", text: $field.value)
                            }
                        }
                    }
                }

                if let err = errorMessage {
                    Text(err).foregroundStyle(.red).font(.caption)
                }
            }
            .formStyle(.grouped)

            HStack {
                Button("Cancel") { dismiss() }
                    .keyboardShortcut(.cancelAction)
                Spacer()
                Button("Save") { save() }
                    .keyboardShortcut(.defaultAction)
                    .disabled(name.isEmpty)
            }
            .padding()
        }
        .frame(width: 450, height: 400)
        .onChange(of: credType) { _, _ in updateFields() }
        .onAppear { updateFields() }
    }

    private func updateFields() {
        let sensitive = Set(["password", "key", "secret_key", "access_key",
                             "token", "refresh_token", "client_secret", "value"])
        let required = requiredFields(for: credType)
        let optional = optionalFields(for: credType)
        fields = required.map { FieldInput(name: $0, sensitive: sensitive.contains($0), optional: false) }
            + optional.map { FieldInput(name: $0, sensitive: sensitive.contains($0), optional: true) }
    }

    private func save() {
        var dict: [String: String] = [:]
        for f in fields where !f.value.isEmpty {
            dict[f.name] = f.value
        }
        let missing = fields.filter { !$0.optional && $0.value.isEmpty }
        if !missing.isEmpty {
            errorMessage = "Missing required fields: \(missing.map(\.name).joined(separator: ", "))"
            return
        }
        let ok = PasskaBridge.addCredentialRaw(
            name: name, type: credType, fields: dict, description: description
        )
        if ok { store.reload(); dismiss() }
        else { errorMessage = "Failed to save credential" }
    }

    private func requiredFields(for type: String) -> [String] {
        switch type {
        case "user_pass": return ["username", "password"]
        case "cookie": return ["value", "domain"]
        case "api_key": return ["key"]
        case "app_secret": return ["access_key", "secret_key"]
        case "token": return ["token"]
        default: return []
        }
    }

    private func optionalFields(for type: String) -> [String] {
        switch type {
        case "user_pass": return ["url"]
        case "cookie": return ["path", "expires"]
        case "api_key": return ["provider", "endpoint"]
        case "app_secret": return ["app_name"]
        case "token": return ["refresh_token", "expires_at", "refresh_url", "client_id", "client_secret"]
        default: return []
        }
    }
}

struct FieldInput: Identifiable {
    let id = UUID()
    let name: String
    let sensitive: Bool
    let optional: Bool
    var value = ""
}
