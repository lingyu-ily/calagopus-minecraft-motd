import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { parseFromApi, serializeForApi } from '@/lib/serialization/api-transform.ts';
import { adminSettingsResponseSchema, enrollmentResponseSchema, extensionSettingsSchema } from '../lib/schemas.ts';

const base = '/api/admin/minecraft-motd';

export async function getAdminSettings() {
  const { data } = await axiosInstance.get(`${base}/settings`);
  return parseFromApi(adminSettingsResponseSchema, data);
}

export async function updateAdminSettings(data: z.infer<typeof extensionSettingsSchema>): Promise<void> {
  await axiosInstance.put(`${base}/settings`, serializeForApi(extensionSettingsSchema, data));
}

export async function createEnrollmentToken(nodeUuid: string) {
  const { data } = await axiosInstance.post(`${base}/nodes/${nodeUuid}/enrollment-token`);
  return parseFromApi(enrollmentResponseSchema, data);
}

export async function revokeAgent(nodeUuid: string): Promise<void> {
  await axiosInstance.delete(`${base}/nodes/${nodeUuid}/agent`);
}
