import { faServer } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { Extension, ExtensionContext } from 'shared';
import { ServerCan } from '@/elements/Can.tsx';
import AdminSettingsPage from './components/AdminSettingsPage.tsx';
import ServerMotdCard from './components/ServerMotdCard.tsx';

function PermissionGuardedServerMotdCard() {
  return (
    <ServerCan action='settings.motd'>
      <ServerMotdCard />
    </ServerCan>
  );
}

class MinecraftMotdExtension extends Extension {
  public cardConfigurationPage: React.FC | null = AdminSettingsPage;
  public cardComponent: React.FC | null = null;
  public cardIcon: React.ReactNode = <FontAwesomeIcon icon={faServer} />;

  public initialize(ctx: ExtensionContext): void {
    ctx.extensionRegistry.pages.server.settings.enterSettingContainers((containers) =>
      containers.appendComponent(PermissionGuardedServerMotdCard),
    );
  }
}

export default new MinecraftMotdExtension();
