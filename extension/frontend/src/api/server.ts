import { z } from 'zod';
import { axiosInstance } from '@/api/axios.ts';
import { parseFromApi, serializeForApi } from '@/lib/serialization/api-transform.ts';
import { serverMotdSettingsSchema } from '../lib/schemas.ts';

export async function getServerMotdSettings(serverUuid: string) {
  const { data } = await axiosInstance.get(`/api/client/servers/${serverUuid}/minecraft-motd`);
  return parseFromApi(serverMotdSettingsSchema, data);
}

export async function updateServerMotdSettings(
  serverUuid: string,
  data: z.infer<typeof serverMotdSettingsSchema>,
): Promise<void> {
  await axiosInstance.put(
    `/api/client/servers/${serverUuid}/minecraft-motd`,
    serializeForApi(serverMotdSettingsSchema, data),
  );
}
