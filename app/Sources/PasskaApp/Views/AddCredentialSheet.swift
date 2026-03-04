import SwiftUI

struct AddCredentialSheet: View {
    @EnvironmentObject var store: CredentialStore
    @Environment(\.dismiss) private var dismiss

    @State private var name = ""
    @State private var credType = "api_key"
    @State private var description = ""
    @State private var fields: [FieldInput] = []
    @State private var sessionPairs: [KVPair] = [KVPair()]
    @State private var sessionDomain = ""
    @State private var errorMessage: String?

    private let typeOptions = [
        ("api_key", "API Key"),
        ("password", "Password"),
        ("session", "Session / Cookie"),
        ("oauth", "OAuth"),
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

                if credType == "session" {
                    sessionSection
                } else {
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
                }

                if credType == "oauth" {
                    Section {
                        Text("After saving, run `passka auth \(name.isEmpty ? "<name>" : name)` to complete authorization.")
                            .font(.caption)
                            .foregroundStyle(.secondary)
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
        .frame(width: 500, height: credType == "oauth" ? 520 : 420)
        .onChange(of: credType) { _, _ in updateFields() }
        .onAppear { updateFields() }
    }

    private var sessionSection: some View {
        Section("Session Data") {
            TextField("Domain (required)", text: $sessionDomain)
            ForEach($sessionPairs) { $pair in
                HStack {
                    TextField("Header name", text: $pair.key)
                        .frame(width: 150)
                    SecureField("Value", text: $pair.value)
                    Button(action: { sessionPairs.removeAll { $0.id == pair.id } }) {
                        Image(systemName: "minus.circle")
                    }
                    .buttonStyle(.borderless)
                    .disabled(sessionPairs.count <= 1)
                }
            }
            Button("Add header/cookie") {
                sessionPairs.append(KVPair())
            }
        }
    }

    private func updateFields() {
        let sensitive = Set(["password", "key", "secret",
                             "token", "refresh_token", "client_secret"])
        let required = requiredFields(for: credType)
        let optional = optionalFields(for: credType)
        fields = required.map { FieldInput(name: $0, sensitive: sensitive.contains($0), optional: false) }
            + optional.map { FieldInput(name: $0, sensitive: sensitive.contains($0), optional: true) }
    }

    private func save() {
        var dict: [String: String] = [:]

        if credType == "session" {
            guard !sessionDomain.isEmpty else {
                errorMessage = "Domain is required"
                return
            }
            dict["domain"] = sessionDomain
            let validPairs = sessionPairs.filter { !$0.key.isEmpty && !$0.value.isEmpty }
            guard !validPairs.isEmpty else {
                errorMessage = "At least one header/cookie entry is required"
                return
            }
            for pair in validPairs {
                dict[pair.key] = pair.value
            }
        } else {
            for f in fields where !f.value.isEmpty {
                dict[f.name] = f.value
            }
            let missing = fields.filter { !$0.optional && $0.value.isEmpty }
            if !missing.isEmpty {
                errorMessage = "Missing required fields: \(missing.map(\.name).joined(separator: ", "))"
                return
            }
        }

        let ok = PasskaBridge.addCredentialRaw(
            name: name, type: credType, fields: dict, description: description
        )
        if ok { store.reload(); dismiss() }
        else { errorMessage = "Failed to save credential" }
    }

    private func requiredFields(for type: String) -> [String] {
        switch type {
        case "api_key": return ["key"]
        case "password": return ["username", "password"]
        case "oauth": return ["authorize_url", "token_url", "client_id", "client_secret"]
        default: return []
        }
    }

    private func optionalFields(for type: String) -> [String] {
        switch type {
        case "api_key": return ["secret", "endpoint"]
        case "password": return ["url"]
        case "oauth": return ["redirect_uri", "scopes"]
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

struct KVPair: Identifiable {
    let id = UUID()
    var key = ""
    var value = ""
}
