import SwiftUI

struct ContentView: View {
    @EnvironmentObject var store: CredentialStore
    @State private var sidebarSelection: SidebarDestination? = .allCredentials
    @State private var selectedEntry: AccountEntry?
    @State private var searchText = ""
    @State private var showingAddSheet = false

    var body: some View {
        NavigationSplitView {
            SidebarView(selection: $sidebarSelection)
        } content: {
            switch sidebarSelection ?? .allCredentials {
            case .allCredentials:
                CredentialListView(
                    selection: $selectedEntry,
                    searchText: $searchText,
                    providerFilter: nil
                )
            case .provider(let provider):
                CredentialListView(
                    selection: $selectedEntry,
                    searchText: $searchText,
                    providerFilter: provider
                )
            case .agents:
                AgentManagementView()
            }
        } detail: {
            if case .allCredentials = sidebarSelection ?? .allCredentials, let entry = selectedEntry {
                CredentialDetailView(entry: entry)
            } else if case .provider = sidebarSelection ?? .allCredentials, let entry = selectedEntry {
                CredentialDetailView(entry: entry)
            } else {
                VStack(spacing: 12) {
                    Image(systemName: detailIcon)
                        .font(.system(size: 42))
                        .foregroundStyle(.secondary)
                    Text(detailTitle)
                        .font(.headline)
                    Text(detailSubtitle)
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
        .onChange(of: sidebarSelection) { _ in
            selectedEntry = nil
            searchText = ""
        }
        .searchable(text: $searchText, prompt: "Search credentials")
    }

    private var detailTitle: String {
        switch sidebarSelection ?? .allCredentials {
        case .allCredentials, .provider:
            return "Select a Credential"
        case .agents:
            return "Manage Agents"
        }
    }

    private var detailSubtitle: String {
        switch sidebarSelection ?? .allCredentials {
        case .allCredentials, .provider:
            return "Passka keeps your local credentials in one place and brokers short-lived leases for agents."
        case .agents:
            return "Register the local AI tools that are allowed to ask Passka for leases."
        }
    }

    private var detailIcon: String {
        switch sidebarSelection ?? .allCredentials {
        case .allCredentials, .provider:
            return "lock.shield"
        case .agents:
            return "person.2"
        }
    }
}
