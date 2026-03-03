import SwiftUI

@main
struct PasskaApp: App {
    @StateObject private var store = CredentialStore()

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environmentObject(store)
                .frame(minWidth: 700, minHeight: 450)
        }
        .windowStyle(.titleBar)
        .defaultSize(width: 900, height: 550)
    }
}
