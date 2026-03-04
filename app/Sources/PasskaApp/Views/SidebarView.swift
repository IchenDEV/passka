import SwiftUI

struct SidebarView: View {
    @EnvironmentObject var store: CredentialStore

    private let types: [(id: String, label: String, icon: String)] = [
        ("api_key", "API Keys", "key.fill"),
        ("password", "Passwords", "person.badge.key.fill"),
        ("session", "Sessions", "globe"),
        ("oauth", "OAuth", "arrow.triangle.2.circlepath"),
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
