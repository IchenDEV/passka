import SwiftUI
import AppKit

struct CredentialDetailView: View {
    @EnvironmentObject var store: CredentialStore
    let entry: CredentialEntry

    @State private var revealedFields: [String: String] = [:]
    @State private var hideTask: Task<Void, Never>?

    private var fields: [String] {
        fieldNamesForType(entry.credType)
    }

    var body: some View {
        Form {
            Section("Metadata") {
                LabeledContent("Name", value: entry.name)
                LabeledContent("Type", value: entry.credType)
                LabeledContent("Description", value: entry.description.isEmpty ? "—" : entry.description)
                LabeledContent("Created", value: entry.createdAt)
            }

            Section("Fields") {
                ForEach(fields, id: \.self) { field in
                    HStack {
                        Text(field).frame(width: 120, alignment: .leading)

                        if let val = revealedFields[field] {
                            Text(val)
                                .textSelection(.enabled)
                                .font(.system(.body, design: .monospaced))
                        } else {
                            Text("••••••••")
                                .foregroundStyle(.secondary)
                        }

                        Spacer()

                        if revealedFields[field] != nil {
                            Button("Copy", systemImage: "doc.on.doc") {
                                copyToClipboard(revealedFields[field] ?? "")
                            }
                            .buttonStyle(.borderless)
                        }

                        Button(revealedFields[field] != nil ? "Hide" : "Reveal",
                               systemImage: revealedFields[field] != nil ? "eye.slash" : "eye") {
                            if revealedFields[field] != nil {
                                revealedFields.removeValue(forKey: field)
                            } else {
                                revealField(field)
                            }
                        }
                        .buttonStyle(.borderless)
                    }
                }
            }

            Section("Environment Variables") {
                ForEach(Array(entry.envVars.sorted(by: { $0.key < $1.key })), id: \.key) { field, envName in
                    LabeledContent(field, value: "$\(envName)")
                }
            }
        }
        .formStyle(.grouped)
        .navigationTitle(entry.name)
        .onChange(of: entry) { _, _ in
            revealedFields.removeAll()
            hideTask?.cancel()
        }
    }

    private func revealField(_ field: String) {
        AuthManager.authenticate(reason: "Reveal credential value") { success in
            guard success else { return }
            if let val = store.getValue(name: entry.name, field: field) {
                revealedFields[field] = val
                scheduleAutoHide()
            }
        }
    }

    private func scheduleAutoHide() {
        hideTask?.cancel()
        hideTask = Task {
            try? await Task.sleep(for: .seconds(30))
            if !Task.isCancelled {
                revealedFields.removeAll()
            }
        }
    }

    private func copyToClipboard(_ value: String) {
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(value, forType: .string)
        Task {
            try? await Task.sleep(for: .seconds(60))
            if NSPasteboard.general.string(forType: .string) == value {
                NSPasteboard.general.clearContents()
            }
        }
    }

    private func fieldNamesForType(_ type: String) -> [String] {
        switch type {
        case "user_pass": return ["username", "password", "url"]
        case "cookie": return ["value", "domain", "path", "expires"]
        case "api_key": return ["key", "provider", "endpoint"]
        case "app_secret": return ["access_key", "secret_key", "app_name"]
        case "token": return ["token", "refresh_token", "expires_at", "refresh_url", "client_id", "client_secret"]
        default: return Array(entry.envVars.keys.sorted())
        }
    }
}
