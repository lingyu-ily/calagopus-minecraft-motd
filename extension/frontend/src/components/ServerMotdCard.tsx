import { faServer } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useEffect, useState } from 'react';
import { httpErrorToHuman } from '@/api/axios.ts';
import Button from '@/elements/buttons/Button.tsx';
import TitleCard from '@/elements/data-display/TitleCard.tsx';
import Switch from '@/elements/input/Switch.tsx';
import Group from '@/elements/layout/Group.tsx';
import Stack from '@/elements/layout/Stack.tsx';
import { useToast } from '@/providers/ToastProvider.tsx';
import { useServerStore } from '@/stores/server.ts';
import { getServerMotdSettings, updateServerMotdSettings } from '../api/server.ts';
import { useExtTranslations } from '../translations.ts';

export default function ServerMotdCard() {
  const { t: tExt } = useExtTranslations();
  const { addToast } = useToast();
  const server = useServerStore((state) => state.server);
  const [enabled, setEnabled] = useState(false);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    getServerMotdSettings(server.uuid)
      .then((settings) => setEnabled(settings.autostartOnJoin))
      .catch((error) => {
        addToast(httpErrorToHuman(error), 'error');
      })
      .finally(() => setLoading(false));
  }, [server.uuid]);

  const save = () => {
    setSaving(true);
    updateServerMotdSettings(server.uuid, { autostartOnJoin: enabled })
      .then(() => addToast(tExt('server.saved', {}), 'success'))
      .catch((error) => {
        addToast(httpErrorToHuman(error), 'error');
      })
      .finally(() => setSaving(false));
  };

  return (
    <TitleCard title={tExt('server.title', {})} icon={<FontAwesomeIcon icon={faServer} />} className='h-full order-55'>
      <Stack h='100%'>
        <Switch
          label={tExt('server.autostart', {})}
          description={tExt('server.description', {})}
          checked={enabled}
          disabled={loading}
          onChange={(event) => setEnabled(event.currentTarget.checked)}
        />
        <Group mt='auto'>
          <Button onClick={save} loading={saving} disabled={loading}>
            {tExt('server.save', {})}
          </Button>
        </Group>
      </Stack>
    </TitleCard>
  );
}
