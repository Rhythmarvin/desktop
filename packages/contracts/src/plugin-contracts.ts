// Generated from ora-contracts. Do not edit.

import type { AgentEvent, AgentMethod, AgentProviderKey, ContentDigest, ContentOwnerId, JsonSafeU64, PluginId, PluginPackageManifest, PluginVersion } from "./plugin-protocol.js";

export type ScanPluginsRequest = { rootIds: Array<string>, };

export type IdentifyPluginRequest = { selectionHandle: string, };

export type InstallPluginRequest = { candidateHandle: string, };

export type RemovePluginDataRequest = { "scope": "currentContentOwner", } | { "scope": "allOwners", confirmationHandle: string, };

export type ApplicationAgentScope = { "type": "global", } | { "type": "project", projectId: string, } | { "type": "worktree", projectId: string, worktreeId: string, };

export type AgentInvocationRequest = { pluginId: PluginId, method: AgentMethod, scope: ApplicationAgentScope, params: unknown, };

export type PluginDiagnosticView = { code: string, message: string, };

export type PluginCatalogItem = { pluginId: PluginId | null, manifest: PluginPackageManifest | null, validity: string, compatibility: string, support: string, integrity: string, diagnostics: Array<PluginDiagnosticView>, };

export type PluginCatalogResponse = { revision: JsonSafeU64, plugins: Array<PluginCatalogItem>, };

export type CandidateSelectionView = { selectionHandle: string, displayName: string, };

export type ScanPluginsResponse = { candidates: Array<CandidateSelectionView>, };

export type IdentifyPluginResponse = { pluginId: PluginId, pluginVersion: PluginVersion, contentDigest: ContentDigest, candidateHandle: string, manifest: PluginPackageManifest, compatibility: string, support: string, diagnostics: Array<PluginDiagnosticView>, };

export type InstallPluginResponse = { pluginId: PluginId, pluginVersion: PluginVersion, contentDigest: ContentDigest, contentOwner: ContentOwnerId, enabled: boolean, };

export type PluginLaunchValueReference = { "type": "hostConfiguration", key: string, } | { "type": "credential", key: string, } | { "type": "discoveredExecutable", provider: AgentProviderKey, } | { "type": "authorizedPath", pathId: string, };

export type PluginEnvironmentBinding = { target: string, value: PluginLaunchValueReference, };

export type SetPluginLaunchGrantRequest = { contentOwner: ContentOwnerId, schemaVersion: number, revision: JsonSafeU64, environment: Array<PluginEnvironmentBinding>, };

export type PluginLaunchGrantView = { pluginId: PluginId, contentOwner: ContentOwnerId, schemaVersion: number, revision: JsonSafeU64, environment: Array<PluginEnvironmentBinding>, };

export type GetPluginLaunchGrantResponse = { grant: PluginLaunchGrantView | null, };

export type PluginActionResponse = Record<symbol, never>;

export type AgentInvocationStreamEnvelope = { "type": "event", event: AgentEvent, } | { "type": "completed", result: unknown, } | { "type": "failed", error: string, };

export type NativePluginSelectionResponse = { selection: CandidateSelectionView | null, };

export type DataRemovalConfirmationResponse = { confirmationHandle: string, };
