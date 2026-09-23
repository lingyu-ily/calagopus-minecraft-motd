import { defineTranslations } from 'shared';

const translations = defineTranslations({
  items: {},
  translations: {
    admin: {
      title: 'Minecraft MOTD',
      description: 'Configure state-aware Java Edition MOTDs and securely enroll node agents.',
      general: 'General settings',
      states: 'State messages',
      agents: 'Node agents',
      enabled: 'Enable MOTD interception',
      allAllocations: 'Intercept all allocations',
      allAllocationsDescription: 'Disabled by default. Primary allocations are safest for Minecraft servers.',
      rotationInterval: 'Rotation interval (seconds)',
      swapLines: 'Swap the first and second MOTD lines',
      gradientEnabled: 'Enable hexadecimal gradient',
      gradientColors: 'Gradient colors',
      excludedEggs: 'Excluded Egg UUIDs',
      excludedServers: 'Excluded server UUIDs',
      favicon: 'Server icon (64×64 PNG)',
      save: 'Save settings',
      saved: 'Minecraft MOTD settings saved.',
      version: 'Version label',
      protocol: 'Protocol override',
      protocolDescription: 'Leave empty to mirror the connecting client protocol.',
      onlinePlayers: 'Displayed online players',
      maxPlayers: 'Displayed maximum players',
      descriptions: 'Rotating MOTDs (one entry separated by a blank line)',
      kickMessage: 'Login disconnect message',
      enrolled: 'Enrolled',
      notEnrolled: 'Not enrolled',
      lastSeen: 'Last seen',
      generateToken: 'Generate enrollment token',
      revoke: 'Revoke agent',
      tokenNotice: 'This one-time token expires in 15 minutes and is shown only now.',
    },
    server: {
      title: 'Minecraft MOTD',
      autostart: 'Start this server when a player tries to join while it is offline',
      description: 'Server-list pings never start the server. Only a real Java login handshake can trigger it.',
      save: 'Save',
      saved: 'Minecraft MOTD setting updated.',
    },
  },
});

export const useExtTranslations = translations.useTranslations.bind(translations);
export const getExtTranslations = translations.getTranslations.bind(translations);
export default translations;
