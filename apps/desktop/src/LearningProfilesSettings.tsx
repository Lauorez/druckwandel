import type { LearnedAssignmentView } from "../../../src/learning/correction-memory.js";

interface LearningProfilesSettingsProps {
  profiles: Array<{ id: string; name: string }>;
  activeProfileId: string;
  learningRuleCount: number;
  canUndo: boolean;
  assignments: LearnedAssignmentView[];
  onSelectProfile: (profileId: string) => void;
  onCreateProfile: () => void;
  onRenameProfile: () => void;
  onDeleteProfile: () => void;
  onClearMemory: () => void;
  onUndo: () => void;
  onToggleAssignment: (id: string, enabled: boolean) => void;
  onRemoveAssignment: (id: string) => void;
}

export function LearningProfilesSettings({
  profiles,
  activeProfileId,
  learningRuleCount,
  canUndo,
  assignments,
  onSelectProfile,
  onCreateProfile,
  onRenameProfile,
  onDeleteProfile,
  onClearMemory,
  onUndo,
  onToggleAssignment,
  onRemoveAssignment,
}: LearningProfilesSettingsProps) {
  return <>
    <p>Beim Speichern merkt sich der Assistent markierte Stellen im ausgewählten Profil. Die Profile gelten ab der nächsten geöffneten Rechnung.</p>
    <div className="learning-profiles">
      <label>Aktives Profil
        <select aria-label="Aktives Erkennungsprofil" value={activeProfileId} onChange={(event) => onSelectProfile(event.target.value)}>
          {profiles.map((profile) => <option key={profile.id} value={profile.id}>{profile.name}</option>)}
        </select>
      </label>
      <button type="button" onClick={onCreateProfile}>Neues Profil</button>
      <button type="button" onClick={onRenameProfile}>Umbenennen</button>
      <button type="button" disabled={profiles.length < 2} onClick={onDeleteProfile}>Profil löschen</button>
      {learningRuleCount > 0 && <button type="button" onClick={onClearMemory}>Gemerkte Ergänzungen löschen</button>}
      {canUndo && <button type="button" onClick={onUndo}>Letzte Bestätigung rückgängig</button>}
    </div>
    {assignments.length > 0 ? <ul className="learned-assignments" aria-label="Gemerkte Zuordnungen">
      {assignments.map((assignment) => <li key={assignment.id} className={assignment.enabled ? "" : "disabled"}>
        <div>
          <strong>{assignment.label}</strong>
          <span>{assignment.context}{assignment.enabled ? "" : " · deaktiviert"}</span>
        </div>
        <div className="learned-assignment-actions">
          <button type="button" onClick={() => onToggleAssignment(assignment.id, !assignment.enabled)}>
            {assignment.enabled ? "Deaktivieren" : "Aktivieren"}
          </button>
          <button type="button" onClick={() => onRemoveAssignment(assignment.id)}>Entfernen</button>
        </div>
      </li>)}
    </ul> : <p className="dialog-empty">In diesem Profil sind noch keine Stellen gemerkt.</p>}
  </>;
}
