import SwiftUI

struct AddCredentialSheet: View {
    @EnvironmentObject var store: CredentialStore
    @Environment(\.dismiss) private var dismiss

    @State private var name = ""
    @State private var provider = "generic_api"
    @State private var authMethod = "api_key"
    @State private var description = ""
    @State private var baseURL = ""

    @State private var apiKey = ""
    @State private var headerName = "Authorization"
    @State private var headerPrefix = "Bearer"

    @State private var authorizeURL = ""
    @State private var tokenURL = ""
    @State private var clientID = ""
    @State private var clientSecret = ""
    @State private var redirectURI = "http://localhost:8477/callback"
    @State private var scopes = ""

    @State private var otpSeed = ""
    @State private var otpIssuer = ""
    @State private var otpAccountName = ""
    @State private var otpDigits = "6"
    @State private var otpPeriod = "30"

    @State private var opaquePairs: [KVPair] = [KVPair()]
    @State private var errorMessage: String?

    private let providers = ["generic_api", "openai", "github", "slack", "feishu"]
    private let authMethods = ["api_key", "oauth", "otp", "opaque"]

    var body: some View {
        VStack(spacing: 0) {
            Form {
                Section("Account") {
                    TextField("Name", text: $name)
                    Picker("Provider", selection: $provider) {
                        ForEach(providers, id: \.self) { provider in
                            Text(provider).tag(provider)
                        }
                    }
                    Picker("Auth Method", selection: $authMethod) {
                        ForEach(authMethods, id: \.self) { method in
                            Text(method).tag(method)
                        }
                    }
                    TextField("Base URL", text: $baseURL)
                    TextField("Description", text: $description)
                }

                switch authMethod {
                case "api_key":
                    Section("API Key Material") {
                        SecureField("API Key", text: $apiKey)
                        TextField("Header Name", text: $headerName)
                        TextField("Header Prefix", text: $headerPrefix)
                    }
                case "oauth":
                    Section("OAuth Config") {
                        TextField("Authorize URL", text: $authorizeURL)
                        TextField("Token URL", text: $tokenURL)
                        TextField("Client ID", text: $clientID)
                        SecureField("Client Secret", text: $clientSecret)
                        TextField("Redirect URI", text: $redirectURI)
                        TextField("Scopes (space-separated)", text: $scopes)
                    }
                    Section {
                        Text("After saving, complete the flow with `passka auth <account_id>` in the CLI.")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                case "otp":
                    Section("OTP Config") {
                        SecureField("Seed (base32)", text: $otpSeed)
                        TextField("Issuer", text: $otpIssuer)
                        TextField("Account Name", text: $otpAccountName)
                        TextField("Digits", text: $otpDigits)
                        TextField("Period Seconds", text: $otpPeriod)
                    }
                default:
                    Section("Opaque Secret Fields") {
                        ForEach($opaquePairs) { $pair in
                            HStack {
                                TextField("Field", text: $pair.key)
                                    .frame(width: 150)
                                SecureField("Value", text: $pair.value)
                                Button(action: { opaquePairs.removeAll { $0.id == pair.id } }) {
                                    Image(systemName: "minus.circle")
                                }
                                .buttonStyle(.borderless)
                                .disabled(opaquePairs.count <= 1)
                            }
                        }
                        Button("Add Field") {
                            opaquePairs.append(KVPair())
                        }
                    }
                }

                if let errorMessage {
                    Text(errorMessage)
                        .foregroundStyle(.red)
                        .font(.caption)
                }
            }
            .formStyle(.grouped)

            HStack {
                Button("Cancel") { dismiss() }
                    .keyboardShortcut(.cancelAction)
                Spacer()
                Button("Save") { save() }
                    .keyboardShortcut(.defaultAction)
                    .disabled(name.isEmpty)
            }
            .padding()
        }
        .frame(width: 520, height: authMethod == "oauth" ? 560 : 460)
    }

    private func save() {
        let ok: Bool
        switch authMethod {
        case "api_key":
            guard !apiKey.isEmpty else {
                errorMessage = "API key is required"
                return
            }
            ok = store.addAPIKeyAccount(
                name: name,
                provider: provider,
                baseURL: baseURL,
                description: description,
                apiKey: apiKey,
                headerName: headerName,
                headerPrefix: headerPrefix
            )
        case "oauth":
            guard !authorizeURL.isEmpty,
                  !tokenURL.isEmpty,
                  !clientID.isEmpty,
                  !clientSecret.isEmpty
            else {
                errorMessage = "Authorize URL, token URL, client ID, and client secret are required"
                return
            }
            ok = store.addOAuthAccount(
                name: name,
                provider: provider,
                baseURL: baseURL,
                description: description,
                authorizeURL: authorizeURL,
                tokenURL: tokenURL,
                clientID: clientID,
                clientSecret: clientSecret,
                redirectURI: redirectURI,
                scopes: scopes
            )
        case "otp":
            guard !otpSeed.isEmpty else {
                errorMessage = "OTP seed is required"
                return
            }
            guard Int(otpDigits) != nil, Int(otpPeriod) != nil else {
                errorMessage = "Digits and period must be numbers"
                return
            }
            ok = store.addOTPAccount(
                name: name,
                provider: provider,
                baseURL: baseURL,
                description: description,
                seed: otpSeed,
                issuer: otpIssuer,
                accountName: otpAccountName,
                digits: otpDigits,
                period: otpPeriod
            )
        default:
            let fields = opaquePairs.reduce(into: [String: String]()) { result, pair in
                if !pair.key.isEmpty && !pair.value.isEmpty {
                    result[pair.key] = pair.value
                }
            }
            guard !fields.isEmpty else {
                errorMessage = "At least one opaque field is required"
                return
            }
            ok = store.addOpaqueAccount(
                name: name,
                provider: provider,
                baseURL: baseURL,
                description: description,
                fields: fields
            )
        }

        if ok {
            dismiss()
        } else {
            errorMessage = "Failed to save account"
        }
    }
}

struct KVPair: Identifiable {
    let id = UUID()
    var key = ""
    var value = ""
}
