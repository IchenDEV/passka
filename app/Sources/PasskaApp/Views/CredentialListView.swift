import SwiftUI

struct CredentialListView: View {
    @EnvironmentObject var store: CredentialStore
    @Binding var selection: AccountEntry?
    @Binding var searchText: String

    private var displayed: [AccountEntry] {
        let base = store.filteredAccounts()
        if searchText.isEmpty { return base }
        let q = searchText.lowercased()
        return base.filter {
            $0.name.lowercased().contains(q)
                || $0.description.lowercased().contains(q)
                || $0.provider.lowercased().contains(q)
        }
    }

    var body: some View {
        List(displayed, selection: $selection) { entry in
            VStack(alignment: .leading, spacing: 6) {
                HStack {
                    Text(entry.name).fontWeight(.medium)
                    Spacer()
                    Text(entry.provider)
                        .font(.caption)
                        .padding(.horizontal, 6)
                        .padding(.vertical, 2)
                        .background(.quaternary)
                        .clipShape(Capsule())
                }
                HStack {
                    Text(entry.authMethod.replacingOccurrences(of: "_", with: " "))
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    if !entry.baseURL.isEmpty {
                        Text(entry.baseURL)
                            .font(.caption)
                            .foregroundStyle(.tertiary)
                            .lineLimit(1)
                    }
                }
                if !entry.description.isEmpty {
                    Text(entry.description)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }
            .padding(.vertical, 2)
            .tag(entry)
        }
        .listStyle(.inset)
        .overlay {
            if displayed.isEmpty {
                VStack(spacing: 10) {
                    Image(systemName: "lock.slash")
                        .font(.system(size: 36))
                        .foregroundStyle(.secondary)
                    Text("No Provider Accounts")
                        .font(.headline)
                    Text("Add an account to let Passka broker access for agents.")
                        .foregroundStyle(.secondary)
                }
            }
        }
    }
}
