import SwiftUI

struct ContentView: View {
    @EnvironmentObject var store: CredentialStore
    @State private var selectedEntry: AccountEntry?
    @State private var searchText = ""
    @State private var showingAddSheet = false

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
                VStack(spacing: 12) {
                    Image(systemName: "lock.shield")
                        .font(.system(size: 42))
                        .foregroundStyle(.secondary)
                    Text("Select an Account")
                        .font(.headline)
                    Text("Passka now brokers provider identities, policies, and audit history for agents.")
                        .foregroundStyle(.secondary)
                        .multilineTextAlignment(.center)
                        .frame(maxWidth: 320)
                }
            }
        }
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                Button("Add Account", systemImage: "plus") {
                    showingAddSheet = true
                }
            }
            ToolbarItem(placement: .automatic) {
                Button("Refresh", systemImage: "arrow.clockwise") {
                    store.reload()
                }
            }
        }
        .sheet(isPresented: $showingAddSheet) {
            AddCredentialSheet()
                .environmentObject(store)
        }
        .searchable(text: $searchText, prompt: "Search provider accounts")
    }
}
