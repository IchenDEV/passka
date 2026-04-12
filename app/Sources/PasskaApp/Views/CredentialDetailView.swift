import SwiftUI
import AppKit

struct CredentialDetailView: View {
    @EnvironmentObject var store: CredentialStore
    let entry: AccountEntry

    @State private var revealedFields: [String: String] = [:]
    @State private var hideTask: Task<Void, Never>?

    private var fields: [String] {
        switch entry.authMethod {
        case "api_key":
            return ["api_key", "header_name", "header_prefix", "secret"]
        case "oauth":
            return [
                "authorize_url",
                "token_url",
                "client_id",
                "client_secret",
                "redirect_uri",
                "scopes",
                "access_token",
                "refresh_token",
                "expires_at",
            ]
        case "otp":
            return ["code", "seed", "issuer", "account_name", "digits", "period"]
        default:
            return ["value"]
        }
    }

    private var relatedAudit: [AuditEntry] {
        store.audits(for: entry)
    }

    private var lastActivity: String {
        store.lastActivity(for: entry)?.timestamp ?? "Never"
    }

    var body: some View {
        Form {
            Section("Credential") {
                LabeledContent("Name", value: entry.name)
                LabeledContent("Service", value: entry.provider)
                LabeledContent("Type", value: entry.authMethod.replacingOccurrences(of: "_", with: " "))
                LabeledContent("Base URL", value: entry.baseURL.isEmpty ? "—" : entry.baseURL)
                LabeledContent("Description", value: entry.description.isEmpty ? "—" : entry.description)
                LabeledContent("Scopes", value: entry.scopes.isEmpty ? "—" : entry.scopes.joined(separator: ", "))
                LabeledContent("Created", value: entry.createdAt)
                LabeledContent("Last Activity", value: lastActivity)
            }

            Section("Stored Secret") {
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

            Section("Audit") {
                if relatedAudit.isEmpty {
                    Text("No audit events yet.")
                        .foregroundStyle(.secondary)
                } else {
                    ForEach(relatedAudit.prefix(8)) { event in
                        VStack(alignment: .leading, spacing: 4) {
                            Text(event.kind)
                                .font(.headline)
                            Text(event.detail)
                                .font(.caption)
                                .foregroundStyle(.secondary)
                            Text(event.timestamp)
                                .font(.caption2)
                                .foregroundStyle(.tertiary)
                        }
                        .padding(.vertical, 2)
                    }
                }
            }

            Section {
                Button("Remove Account", role: .destructive) {
                    store.remove(accountId: entry.id)
                }
            }
        }
        .formStyle(.grouped)
        .navigationTitle(entry.name)
        .onChange(of: entry.id) { _ in
            revealedFields.removeAll()
            hideTask?.cancel()
        }
    }

    private func revealField(_ field: String) {
        AuthManager.authenticate(reason: "Reveal broker-managed sensitive material") { success in
            guard success else { return }
            if let val = store.revealValue(accountId: entry.id, field: field), !val.isEmpty {
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
}
