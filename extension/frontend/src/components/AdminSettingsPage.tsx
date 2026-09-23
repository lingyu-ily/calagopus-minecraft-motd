import { Alert, Badge, Code, Divider, SimpleGrid, Tabs } from '@mantine/core';
import { useEffect, useMemo, useState } from 'react';
import { httpErrorToHuman } from '@/api/axios.ts';
import Button from '@/elements/buttons/Button.tsx';
import TitleCard from '@/elements/data-display/TitleCard.tsx';
import NumberInput from '@/elements/input/NumberInput.tsx';
import Switch from '@/elements/input/Switch.tsx';
import TextArea from '@/elements/input/TextArea.tsx';
import TextInput from '@/elements/input/TextInput.tsx';
import Group from '@/elements/layout/Group.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import { useToast } from '@/providers/ToastProvider.tsx';
import { createEnrollmentToken, getAdminSettings, revokeAgent, updateAdminSettings } from '../api/admin.ts';
import { AgentNode, ExtensionSettings, extensionSettingsSchema } from '../lib/schemas.ts';
import { useExtTranslations } from '../translations.ts';

const stateNames = [
  'offline',
  'starting',
  'stopping',
  'suspended',
  'node_maintenance',
  'transferring',
  'installing',
  'install_failed',
  'restoring_backup',
  'backup_restore_failed',
  'node_unreachable',
] as const;

const humanState = (state: string) =>
  state
    .split('_')
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
    .join(' ');

const uuidList = (value: string) =>
  value
    .split(/[\s,]+/)
    .map((entry) => entry.trim())
    .filter(Boolean);

export default function AdminSettingsPage() {
  const { t: tExt } = useExtTranslations();
  const { addToast } = useToast();
  const [settings, setSettings] = useState<ExtensionSettings | null>(null);
  const [nodes, setNodes] = useState<AgentNode[]>([]);
  const [activeState, setActiveState] = useState<(typeof stateNames)[number]>('offline');
  const [saving, setSaving] = useState(false);
  const [token, setToken] = useState<{ node: string; value: string; expires: Date } | null>(null);

  const reload = () =>
    getAdminSettings()
      .then((data) => {
        setSettings(data.settings);
        setNodes(data.nodes);
      })
      .catch((error) => {
        addToast(httpErrorToHuman(error), 'error');
      });

  useEffect(() => {
    reload();
  }, []);

  const currentState = useMemo(() => settings?.states[activeState], [settings, activeState]);

  const updateState = (patch: Partial<NonNullable<typeof currentState>>) => {
    setSettings((current) => {
      if (!current) return current;
      return {
        ...current,
        states: {
          ...current.states,
          [activeState]: { ...current.states[activeState], ...patch },
        },
      };
    });
  };

  const save = () => {
    if (!settings) return;
    const parsed = extensionSettingsSchema.safeParse(settings);
    if (!parsed.success) {
      addToast(parsed.error.issues.map((issue) => issue.message).join('; '), 'error');
      return;
    }
    setSaving(true);
    updateAdminSettings(parsed.data)
      .then(() => addToast(tExt('admin.saved', {}), 'success'))
      .catch((error) => {
        addToast(httpErrorToHuman(error), 'error');
      })
      .finally(() => setSaving(false));
  };

  const selectIcon = (file: File | null) => {
    if (!file || !settings) return;
    if (file.type !== 'image/png') {
      addToast('The server icon must be a PNG file.', 'error');
      return;
    }
    const reader = new FileReader();
    reader.onload = () => {
      const url = String(reader.result);
      const image = new Image();
      image.onload = () => {
        if (image.width !== 64 || image.height !== 64) {
          addToast('The server icon must be exactly 64×64 pixels.', 'error');
          return;
        }
        setSettings((current) => (current ? { ...current, faviconBase64: url.slice(url.indexOf(',') + 1) } : current));
      };
      image.src = url;
    };
    reader.readAsDataURL(file);
  };

  if (!settings || !currentState) {
    return <TitleCard title={tExt('admin.title', {})}>Loading…</TitleCard>;
  }

  return (
    <Stack gap='md'>
      <Alert title={tExt('admin.title', {})}>{tExt('admin.description', {})}</Alert>

      <TitleCard title={tExt('admin.general', {})}>
        <Stack>
          <SimpleGrid cols={{ base: 1, md: 2 }}>
            <Switch
              label={tExt('admin.enabled', {})}
              checked={settings.enabled}
              onChange={(event) => setSettings({ ...settings, enabled: event.currentTarget.checked })}
            />
            <Switch
              label={tExt('admin.allAllocations', {})}
              description={tExt('admin.allAllocationsDescription', {})}
              checked={settings.allAllocations}
              onChange={(event) => setSettings({ ...settings, allAllocations: event.currentTarget.checked })}
            />
            <Switch
              label={tExt('admin.swapLines', {})}
              checked={settings.swapLines}
              onChange={(event) => setSettings({ ...settings, swapLines: event.currentTarget.checked })}
            />
            <Switch
              label={tExt('admin.gradientEnabled', {})}
              checked={settings.gradientEnabled}
              onChange={(event) => setSettings({ ...settings, gradientEnabled: event.currentTarget.checked })}
            />
            <NumberInput
              label={tExt('admin.rotationInterval', {})}
              min={1}
              max={3600}
              value={settings.rotationIntervalSeconds}
              onChange={(value) => setSettings({ ...settings, rotationIntervalSeconds: Number(value) || 10 })}
            />
            <TextInput
              label={tExt('admin.gradientColors', {})}
              value={settings.gradientColors.join(', ')}
              onChange={(event) =>
                setSettings({
                  ...settings,
                  gradientColors: event.currentTarget.value
                    .split(',')
                    .map((color) => color.trim())
                    .filter(Boolean),
                })
              }
            />
          </SimpleGrid>
          <SimpleGrid cols={{ base: 1, md: 2 }}>
            <TextArea
              label={tExt('admin.excludedEggs', {})}
              autosize
              minRows={3}
              value={settings.excludedEggUuids.join('\n')}
              onChange={(event) => setSettings({ ...settings, excludedEggUuids: uuidList(event.currentTarget.value) })}
            />
            <TextArea
              label={tExt('admin.excludedServers', {})}
              autosize
              minRows={3}
              value={settings.excludedServerUuids.join('\n')}
              onChange={(event) =>
                setSettings({ ...settings, excludedServerUuids: uuidList(event.currentTarget.value) })
              }
            />
          </SimpleGrid>
          <label>
            <span className='block text-sm font-medium mb-1'>{tExt('admin.favicon', {})}</span>
            <input type='file' accept='image/png' onChange={(event) => selectIcon(event.target.files?.[0] ?? null)} />
          </label>
        </Stack>
      </TitleCard>

      <TitleCard title={tExt('admin.states', {})}>
        <Tabs value={activeState} onChange={(value) => value && setActiveState(value as typeof activeState)}>
          <Tabs.List>
            {stateNames.map((state) => (
              <Tabs.Tab key={state} value={state}>
                {humanState(state)}
              </Tabs.Tab>
            ))}
          </Tabs.List>
        </Tabs>
        <Stack mt='md'>
          <SimpleGrid cols={{ base: 1, md: 2, xl: 4 }}>
            <TextInput
              label={tExt('admin.version', {})}
              value={currentState.version}
              onChange={(event) => updateState({ version: event.currentTarget.value })}
            />
            <NumberInput
              label={tExt('admin.protocol', {})}
              description={tExt('admin.protocolDescription', {})}
              value={currentState.protocol ?? ''}
              onChange={(value) => updateState({ protocol: value === '' ? null : Number(value) })}
            />
            <NumberInput
              label={tExt('admin.onlinePlayers', {})}
              min={0}
              value={currentState.onlinePlayers}
              onChange={(value) => updateState({ onlinePlayers: Number(value) || 0 })}
            />
            <NumberInput
              label={tExt('admin.maxPlayers', {})}
              min={0}
              value={currentState.maxPlayers}
              onChange={(value) => updateState({ maxPlayers: Number(value) || 0 })}
            />
          </SimpleGrid>
          <TextArea
            label={tExt('admin.descriptions', {})}
            description='Separate rotating entries with a line containing only ---.'
            autosize
            minRows={6}
            value={currentState.descriptions.join('\n---\n')}
            onChange={(event) =>
              updateState({
                descriptions: event.currentTarget.value
                  .split(/\n---\n/)
                  .map((entry) => entry.trim())
                  .filter(Boolean),
              })
            }
          />
          <TextArea
            label={tExt('admin.kickMessage', {})}
            autosize
            minRows={4}
            value={currentState.kickMessage}
            onChange={(event) => updateState({ kickMessage: event.currentTarget.value })}
          />
        </Stack>
      </TitleCard>

      <TitleCard title={tExt('admin.agents', {})}>
        <Stack>
          {nodes.map((node) => (
            <div key={node.nodeUuid} className='rounded-md border border-(--mantine-color-default-border) p-3'>
              <Group justify='space-between' align='start'>
                <div>
                  <div className='font-medium'>{node.nodeName}</div>
                  <div className='text-xs text-(--mantine-color-dimmed)'>{node.nodeUuid}</div>
                  {node.lastSeen && (
                    <div className='text-xs mt-1'>
                      {tExt('admin.lastSeen', {})}: {node.lastSeen.toLocaleString()} · {node.version ?? 'unknown'}
                    </div>
                  )}
                </div>
                <Badge color={node.enrolled && !node.revokedAt ? 'green' : 'gray'}>
                  {node.enrolled && !node.revokedAt ? tExt('admin.enrolled', {}) : tExt('admin.notEnrolled', {})}
                </Badge>
              </Group>
              <Group mt='sm'>
                <Button
                  size='xs'
                  onClick={() =>
                    createEnrollmentToken(node.nodeUuid)
                      .then((created) =>
                        setToken({ node: node.nodeName, value: created.enrollmentToken, expires: created.expiresAt }),
                      )
                      .catch((error) => {
                        addToast(httpErrorToHuman(error), 'error');
                      })
                  }
                >
                  {tExt('admin.generateToken', {})}
                </Button>
                {node.enrolled && !node.revokedAt && (
                  <Button
                    size='xs'
                    color='red'
                    onClick={() =>
                      revokeAgent(node.nodeUuid)
                        .then(reload)
                        .catch((error) => {
                          addToast(httpErrorToHuman(error), 'error');
                        })
                    }
                  >
                    {tExt('admin.revoke', {})}
                  </Button>
                )}
              </Group>
            </div>
          ))}
          {token && (
            <Alert color='yellow' title={`${token.node} — ${tExt('admin.tokenNotice', {})}`}>
              <Code block className='minecraft-motd-token'>
                {token.value}
              </Code>
              <div className='text-xs mt-2'>{token.expires.toLocaleString()}</div>
            </Alert>
          )}
        </Stack>
      </TitleCard>

      <Divider />
      <Group>
        <Button onClick={save} loading={saving}>
          {tExt('admin.save', {})}
        </Button>
      </Group>
    </Stack>
  );
}
