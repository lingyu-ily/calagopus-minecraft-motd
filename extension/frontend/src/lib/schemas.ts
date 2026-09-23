import { z } from 'zod';

export const motdStateSchema = z.object({
  version: z.string().min(1).max(128),
  protocol: z.number().int().nullable(),
  onlinePlayers: z.number().int().min(0),
  maxPlayers: z.number().int().min(0),
  descriptions: z.array(z.string().min(1).max(32767)).min(1),
  kickMessage: z.string().min(1).max(32767),
});

export const extensionSettingsSchema = z.object({
  enabled: z.boolean(),
  allAllocations: z.boolean(),
  rotationIntervalSeconds: z.number().int().min(1).max(3600),
  swapLines: z.boolean(),
  gradientEnabled: z.boolean(),
  gradientColors: z.array(z.string().regex(/^#[0-9a-fA-F]{6}$/)),
  faviconBase64: z.string().nullable(),
  excludedEggUuids: z.array(z.string().uuid()),
  excludedServerUuids: z.array(z.string().uuid()),
  states: z.record(z.string(), motdStateSchema),
});

export const agentNodeSchema = z.object({
  nodeUuid: z.string().uuid(),
  nodeName: z.string(),
  enrolled: z.boolean(),
  version: z.string().nullable(),
  lastSeen: z.coerce.date().nullable(),
  revokedAt: z.coerce.date().nullable(),
});

export const adminSettingsResponseSchema = z.object({
  settings: extensionSettingsSchema,
  nodes: z.array(agentNodeSchema),
});

export const enrollmentResponseSchema = z.object({
  nodeUuid: z.string().uuid(),
  enrollmentToken: z.string(),
  expiresAt: z.coerce.date(),
});

export const serverMotdSettingsSchema = z.object({
  autostartOnJoin: z.boolean(),
});

export type ExtensionSettings = z.infer<typeof extensionSettingsSchema>;
export type AgentNode = z.infer<typeof agentNodeSchema>;
