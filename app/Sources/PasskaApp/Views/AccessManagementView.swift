import SwiftUI

struct AccessManagementView: View {
    @EnvironmentObject var store: CredentialStore

    @State private var selectedAccountId = ""
    @State private var selectedAgentId = ""
    @State private var leaseSeconds = "300"
    @State private var environments = ""
    @State private var description = ""
    @State private var errorMessage: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            Form {
                Section("Authorize Account Access") {
                    Picker("Credential Account", selection: $selectedAccountId) {
                        Text("Select an account").tag("")
                        ForEach(store.accounts) { account in
                            Text(account.name).tag(account.id)
                        }
                    }

                    Picker("Agent", selection: $selectedAgentId) {
                        Text("Select an agent").tag("")
                        ForEach(store.agentPrincipals) { agent in
                            Text(agent.name).tag(agent.id)
                        }
                    }

                    TextField("Lease Seconds", text: $leaseSeconds)
                    TextField("Environments (comma-separated, optional)", text: $environments)
                    TextField("Description (optional)", text: $description)

                    HStack {
                        Spacer()
                        Button("Authorize Access") {
                            authorize()
                        }
                        .disabled(selectedAccountId.isEmpty || selectedAgentId.isEmpty)
                    }
                }

                if let errorMessage {
                    Section {
                        Text(errorMessage)
                            .foregroundStyle(.red)
                            .font(.caption)
                    }
                }
            }
            .formStyle(.grouped)
            .frame(maxHeight: 280)

            VStack(alignment: .leading, spacing: 12) {
                Text("Current Access")
                    .font(.headline)

                if store.authorizations.isEmpty {
                    EmptyStateView(
                        systemImage: "key.slash",
                        title: "No Access Rules Yet",
                        message: "Authorize a stored credential account for an agent so it can request short-lived leases."
                    )
                } else {
                    List(store.authorizations) { authorization in
                        VStack(alignment: .leading, spacing: 4) {
                            HStack {
                                Text(store.accountName(for: authorization.accountId))
                                    .fontWeight(.medium)
                                Image(systemName: "arrow.right")
                                    .foregroundStyle(.tertiary)
                                Text(store.principalName(for: authorization.principalId))
                                Spacer()
                                Text("\(authorization.maxLeaseSeconds)s")
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }

                            if !authorization.environments.isEmpty {
                                Text("Environments: \(authorization.environments.joined(separator: ", "))")
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }

                            if !authorization.description.isEmpty {
                                Text(authorization.description)
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }

                            Text("Created \(authorization.createdAt)")
                                .font(.caption2)
                                .foregroundStyle(.tertiary)
                        }
                        .padding(.vertical, 2)
                    }
                    .listStyle(.inset)
                }
            }
        }
        .padding()
        .navigationTitle("Access")
        .onAppear {
            seedDefaultsIfNeeded()
        }
        .onChange(of: store.accounts.count) { _ in
            seedDefaultsIfNeeded()
        }
        .onChange(of: store.agentPrincipals.count) { _ in
            seedDefaultsIfNeeded()
        }
    }

    private func seedDefaultsIfNeeded() {
        if selectedAccountId.isEmpty, let account = store.accounts.first {
            selectedAccountId = account.id
        }
        if selectedAgentId.isEmpty, let agent = store.agentPrincipals.first {
            selectedAgentId = agent.id
        }
    }

    private func authorize() {
        guard let seconds = Int(leaseSeconds), seconds > 0 else {
            errorMessage = "Lease seconds must be a positive number"
            return
        }

        let environmentValues = environments
            .split(separator: ",")
            .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
            .filter { !$0.isEmpty }

        let ok = store.authorize(
            accountId: selectedAccountId,
            agentPrincipalId: selectedAgentId,
            leaseSeconds: seconds,
            environments: environmentValues,
            description: description
        )

        if ok {
            errorMessage = nil
            description = ""
            environments = ""
        } else {
            errorMessage = "Failed to authorize access"
        }
    }
}
