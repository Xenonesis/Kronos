import { memo } from "react";
import { AddHostMethodModal } from "@/components/add-host-method-modal";
import { AddHostModal } from "@/components/add-host-modal";
import { AddRemoteSshHostModal } from "@/components/add-remote-ssh-host-modal";
import { PairLinkModal } from "@/components/pair-link-modal";

export interface SettingsAddHostModalsProps {
  isAddHostMethodVisible: boolean;
  isDirectHostVisible: boolean;
  isRemoteSshVisible: boolean;
  isPasteLinkVisible: boolean;
  onClose: () => void;
  onCancel: () => void;
  onDirectConnection: () => void;
  onRemoteSsh: () => void;
  onPasteLink: () => void;
  onScanQr: () => void;
  onHostAdded: (data: { serverId: string }) => void;
}

export const SettingsAddHostModals = memo(function SettingsAddHostModals({
  isAddHostMethodVisible,
  isDirectHostVisible,
  isRemoteSshVisible,
  isPasteLinkVisible,
  onClose,
  onCancel,
  onDirectConnection,
  onRemoteSsh,
  onPasteLink,
  onScanQr,
  onHostAdded,
}: SettingsAddHostModalsProps) {
  return (
    <>
      <AddHostMethodModal
        visible={isAddHostMethodVisible}
        onClose={onClose}
        onDirectConnection={onDirectConnection}
        onRemoteSsh={onRemoteSsh}
        onPasteLink={onPasteLink}
        onScanQr={onScanQr}
      />
      <AddHostModal
        visible={isDirectHostVisible}
        onClose={onClose}
        onCancel={onCancel}
        onSaved={onHostAdded}
      />
      <AddRemoteSshHostModal
        visible={isRemoteSshVisible}
        onClose={onClose}
        onCancel={onCancel}
        onSaved={onHostAdded}
      />
      <PairLinkModal
        visible={isPasteLinkVisible}
        onClose={onClose}
        onCancel={onCancel}
        onSaved={onHostAdded}
      />
    </>
  );
});
