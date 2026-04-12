import SwiftUI

enum SidebarDestination: Hashable {
    case allCredentials
    case provider(String)
    case agents
    case access
}

struct SidebarView: View {
    @EnvironmentObject var store: CredentialStore
    @Binding var selection: SidebarDestination?

    private var providers: [String] {
        Array(Set(store.accounts.map(\.provider))).sorted()
    }

    var body: some View {
        List(selection: $selection) {
            Section("Credentials") {
                NavigationLink(value: SidebarDestination.allCredentials) {
                    Label("All Credentials", systemImage: "lock.doc")
                        .badge(store.accounts.count)
                }

                ForEach(providers, id: \.self) { provider in
                    NavigationLink(value: SidebarDestination.provider(provider)) {
                        Label(provider, systemImage: icon(for: provider))
                            .badge(store.accounts.filter { $0.provider == provider }.count)
                    }
                }
            }

            Section("Management") {
                NavigationLink(value: SidebarDestination.agents) {
                    Label("Agents", systemImage: "person.2")
                        .badge(store.agentPrincipals.count)
                }

                NavigationLink(value: SidebarDestination.access) {
                    Label("Access", systemImage: "key.viewfinder")
                        .badge(store.authorizations.count)
                }
            }
        }
        .listStyle(.sidebar)
        .navigationTitle("Passka Vault")
    }

    private func icon(for provider: String) -> String {
        switch provider {
        case "openai": return "sparkles"
        case "github": return "chevron.left.forwardslash.chevron.right"
        case "slack": return "bubble.left.and.bubble.right"
        case "feishu": return "building.2"
        default: return "network"
        }
    }
}
