// The Settings screen (UI-4 comes later): for now the privacy statement.
import { strings } from "./strings";

export function Settings() {
  return (
    <div className="settings">
      <section aria-labelledby="privacy-title">
        <h2 id="privacy-title">{strings.settings.privacyTitle}</h2>
        <p>{strings.settings.privacy}</p>
      </section>
      <p className="muted">{strings.settings.moreLater}</p>
    </div>
  );
}
