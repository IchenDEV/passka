import SwiftUI

struct SidebarView: View {
    @EnvironmentObject var store: CredentialStore

    private let types: [(id: String, label: String, icon: String)] = [
        ("api_key", "API Keys", "key.fill"),
        ("user_pass", "Passwords", "person.badge.key.fill"),
        ("cookie", "Cookies", "globe"),
        ("token", "Tokens", "arrow.triangle.2.circlepath"),
        ("app_secret", "App Secrets", "lock.shield.fill"),
        ("custom", "Custom", "ellipsis.rectangle.fill"),
    ]

    var body: some View {
        List(selection: $store.selectedType) {
            NavigationLink(value: nil as String?) {
                Label {
                    Text("All")
                } icon: {
                    Image(systemName: "tray.full.fill")
                }
                .badge(store.entries.count)
            }

            Section("Types") {
                ForEach(types, id: \.id) { item in
                    NavigationLink(value: Optional(item.id)) {
                        Label(item.label, systemImage: item.icon)
                            .badge(store.entries.filter { $0.credType == item.id }.count)
                    }
                }
            }
        }
        .listStyle(.sidebar)
        .navigationTitle("Passka")
    }
}
