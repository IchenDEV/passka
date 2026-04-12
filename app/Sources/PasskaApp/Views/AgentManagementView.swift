import SwiftUI

struct AgentManagementView: View {
    @EnvironmentObject var store: CredentialStore

    @State private var name = ""
    @State private var description = ""
    @State private var errorMessage: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            Form {
                Section("Register Agent") {
                    TextField("Agent Name", text: $name)
                    TextField("Description", text: $description)

                    HStack {
                        Spacer()
                        Button("Register Agent") {
                            save()
                        }
                        .disabled(name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
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
            .frame(maxHeight: 220)

            VStack(alignment: .leading, spacing: 12) {
                Text("Registered Agents")
                    .font(.headline)

                if store.agentPrincipals.isEmpty {
                    EmptyStateView(
                        systemImage: "person.crop.circle.badge.plus",
                        title: "No Agents Yet",
                        message: "Register a local AI tool here, then authorize credential accounts for it in Access."
                    )
                } else {
                    List(store.agentPrincipals) { agent in
                        VStack(alignment: .leading, spacing: 4) {
                            HStack {
                                Text(agent.name)
                                    .fontWeight(.medium)
                                Spacer()
                                Text(agent.id)
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }
                            if !agent.description.isEmpty {
                                Text(agent.description)
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }
                            Text("Created \(agent.createdAt)")
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
        .navigationTitle("Agents")
    }

    private func save() {
        let trimmedName = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedName.isEmpty else {
            errorMessage = "Agent name is required"
            return
        }

        let ok = store.addAgent(name: trimmedName, description: description)
        if ok {
            name = ""
            description = ""
            errorMessage = nil
        } else {
            errorMessage = "Failed to register agent"
        }
    }
}
