import SwiftUI

struct SidebarView: View {
    @EnvironmentObject var store: CredentialStore

    private var providers: [String] {
        Array(Set(store.accounts.map(\.provider))).sorted()
    }

    var body: some View {
        List(selection: $store.selectedProvider) {
            NavigationLink(value: nil as String?) {
                Label("All Accounts", systemImage: "person.text.rectangle")
                    .badge(store.accounts.count)
            }

            Section("Providers") {
                ForEach(providers, id: \.self) { provider in
                    NavigationLink(value: Optional(provider)) {
                        Label(provider, systemImage: icon(for: provider))
                            .badge(store.accounts.filter { $0.provider == provider }.count)
                    }
                }
            }
        }
        .listStyle(.sidebar)
        .navigationTitle("Passka Broker")
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
