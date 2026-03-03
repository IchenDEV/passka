import SwiftUI

struct ContentView: View {
    @EnvironmentObject var store: CredentialStore
    @State private var selectedEntry: CredentialEntry?
    @State private var searchText = ""

    var body: some View {
        NavigationSplitView {
            SidebarView()
        } content: {
            CredentialListView(
                selection: $selectedEntry,
                searchText: $searchText
            )
        } detail: {
            if let entry = selectedEntry {
                CredentialDetailView(entry: entry)
            } else {
                Text("Select a credential")
                    .foregroundStyle(.secondary)
            }
        }
        .searchable(text: $searchText, prompt: "Search credentials")
    }
}
