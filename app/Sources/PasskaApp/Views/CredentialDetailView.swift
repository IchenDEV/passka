import SwiftUI

struct CredentialDetailView: View {
    @EnvironmentObject var store: CredentialStore
    let entry: AccountEntry

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
    }
}
