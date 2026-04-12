import Foundation
import Combine
import PasskaBridge

struct AccountEntry: Identifiable, Hashable {
    let id: String
    let name: String
    let provider: String
    let authMethod: String
    let baseURL: String
    let description: String
    let scopes: [String]
    let createdAt: String

    init(from dict: [String: Any]) {
        self.id = dict["id"] as? String ?? ""
        self.name = dict["name"] as? String ?? ""
        self.provider = dict["provider"] as? String ?? "generic_api"
        self.authMethod = dict["auth_method"] as? String ?? "opaque"
        self.baseURL = dict["base_url"] as? String ?? ""
        self.description = dict["description"] as? String ?? ""
        self.scopes = dict["scopes"] as? [String] ?? []
        self.createdAt = dict["created_at"] as? String ?? ""
    }
}

struct PrincipalEntry: Identifiable, Hashable {
    let id: String
    let name: String
    let kind: String
    let description: String
    let createdAt: String

    init(from dict: [String: Any]) {
        self.id = dict["id"] as? String ?? ""
        self.name = dict["name"] as? String ?? ""
        self.kind = dict["kind"] as? String ?? ""
        self.description = dict["description"] as? String ?? ""
        self.createdAt = dict["created_at"] as? String ?? ""
    }
}

struct AuthorizationEntry: Identifiable, Hashable {
    let id: String
    let principalId: String
    let accountId: String
    let environments: [String]
    let maxLeaseSeconds: Int
    let description: String
    let createdAt: String

    init(from dict: [String: Any]) {
        self.id = dict["id"] as? String ?? ""
        self.principalId = dict["principal_id"] as? String ?? ""
        self.accountId = dict["account_id"] as? String ?? ""
        self.environments = dict["environments"] as? [String] ?? []
        self.maxLeaseSeconds = dict["max_lease_seconds"] as? Int ?? 0
        self.description = dict["description"] as? String ?? ""
        self.createdAt = dict["created_at"] as? String ?? ""
    }
}

struct AuditEntry: Identifiable, Hashable {
    let id: String
    let timestamp: String
    let actorPrincipalId: String
    let kind: String
    let outcome: String
    let resource: String
    let detail: String

    init(from dict: [String: Any]) {
        self.id = dict["id"] as? String ?? UUID().uuidString
        self.timestamp = dict["timestamp"] as? String ?? ""
        self.actorPrincipalId = dict["actor_principal_id"] as? String ?? ""
        self.kind = dict["kind"] as? String ?? ""
        self.outcome = dict["outcome"] as? String ?? ""
        self.resource = dict["resource"] as? String ?? ""
        self.detail = dict["detail"] as? String ?? ""
    }
}

final class CredentialStore: ObservableObject {
    @Published var accounts: [AccountEntry] = []
    @Published var principals: [PrincipalEntry] = []
    @Published var authorizations: [AuthorizationEntry] = []
    @Published var auditEvents: [AuditEntry] = []

    init() {
        reload()
    }

    func reload() {
        accounts = PasskaBridge.listAccounts().map(AccountEntry.init(from:))
        principals = PasskaBridge.listPrincipals().map(PrincipalEntry.init(from:))
        authorizations = PasskaBridge.listAuthorizations().map(AuthorizationEntry.init(from:))
        auditEvents = PasskaBridge.listAuditEvents(limit: 50).map(AuditEntry.init(from:))
    }

    func revealValue(accountId: String, field: String) -> String? {
        PasskaBridge.revealAccountField(accountId: accountId, field: field)
    }

    func remove(accountId: String) {
        _ = PasskaBridge.removeAccount(accountId: accountId)
        reload()
    }

    var agentPrincipals: [PrincipalEntry] {
        principals.filter { $0.kind == "agent" }.sorted { $0.name.localizedCaseInsensitiveCompare($1.name) == .orderedAscending }
    }

    func addAgent(name: String, description: String) -> Bool {
        let ok = PasskaBridge.addPrincipal(name: name, kind: "agent", description: description)
        if ok { reload() }
        return ok
    }

    func authorize(accountId: String, agentPrincipalId: String, leaseSeconds: Int, environments: [String], description: String) -> Bool {
        let ok = PasskaBridge.authorizeAccount(
            accountId: accountId,
            agentPrincipalId: agentPrincipalId,
            leaseSeconds: leaseSeconds,
            environments: environments,
            description: description
        )
        if ok { reload() }
        return ok
    }

    func accountName(for accountId: String) -> String {
        accounts.first(where: { $0.id == accountId })?.name ?? accountId
    }

    func principalName(for principalId: String) -> String {
        principals.first(where: { $0.id == principalId })?.name ?? principalId
    }

    func authorizations(for account: AccountEntry) -> [AuthorizationEntry] {
        authorizations.filter { $0.accountId == account.id }
    }

    func addAPIKeyAccount(
        name: String,
        provider: String,
        baseURL: String,
        description: String,
        apiKey: String,
        headerName: String,
        headerPrefix: String
    ) -> Bool {
        let ok = PasskaBridge.registerAPIKeyAccount(
            name: name,
            provider: provider,
            baseURL: baseURL,
            description: description,
            apiKey: apiKey,
            headerName: headerName,
            headerPrefix: headerPrefix
        )
        if ok { reload() }
        return ok
    }

    func addOAuthAccount(
        name: String,
        provider: String,
        baseURL: String,
        description: String,
        authorizeURL: String,
        tokenURL: String,
        clientID: String,
        clientSecret: String,
        redirectURI: String,
        scopes: String
    ) -> Bool {
        let ok = PasskaBridge.registerOAuthAccount(
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
        if ok { reload() }
        return ok
    }

    func addOpaqueAccount(
        name: String,
        provider: String,
        baseURL: String,
        description: String,
        fields: [String: String]
    ) -> Bool {
        let ok = PasskaBridge.registerOpaqueAccount(
            name: name,
            provider: provider,
            baseURL: baseURL,
            description: description,
            fields: fields
        )
        if ok { reload() }
        return ok
    }

    func addOTPAccount(
        name: String,
        provider: String,
        baseURL: String,
        description: String,
        seed: String,
        issuer: String,
        accountName: String,
        digits: String,
        period: String
    ) -> Bool {
        let ok = PasskaBridge.registerOTPAccount(
            name: name,
            provider: provider,
            baseURL: baseURL,
            description: description,
            seed: seed,
            issuer: issuer,
            accountName: accountName,
            digits: digits,
            period: period
        )
        if ok { reload() }
        return ok
    }

    func audits(for account: AccountEntry) -> [AuditEntry] {
        auditEvents.filter { $0.resource.contains(account.id) || $0.resource.contains(account.name) }
    }

    func lastActivity(for account: AccountEntry) -> AuditEntry? {
        audits(for: account).first
    }
}
