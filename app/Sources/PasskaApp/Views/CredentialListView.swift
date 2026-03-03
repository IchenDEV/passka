import SwiftUI

struct CredentialListView: View {
    @EnvironmentObject var store: CredentialStore
    @Binding var selection: CredentialEntry?
    @Binding var searchText: String

    private var displayed: [CredentialEntry] {
        let base = store.filtered()
        if searchText.isEmpty { return base }
        let q = searchText.lowercased()
        return base.filter {
            $0.name.lowercased().contains(q)
                || $0.description.lowercased().contains(q)
        }
    }

    var body: some View {
        List(displayed, selection: $selection) { entry in
            VStack(alignment: .leading, spacing: 4) {
                HStack {
                    Text(entry.name).fontWeight(.medium)
                    Spacer()
                    Text(entry.credType)
                        .font(.caption)
                        .padding(.horizontal, 6)
                        .padding(.vertical, 2)
                        .background(.quaternary)
                        .clipShape(Capsule())
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
                ContentUnavailableView(
                    "No Credentials",
                    systemImage: "key.slash",
                    description: Text("Add credentials via CLI:\npasska add <name> --type <type>")
                )
            }
        }
    }
}
