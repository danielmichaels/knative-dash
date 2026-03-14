interface Props {
  namespaces: string[]
  selected: string
  onChange: (ns: string) => void
  disabled?: boolean
}

export function NamespaceSelector({ namespaces, selected, onChange, disabled }: Props) {
  if (namespaces.length === 0) {
    return <p className="ns-empty">No namespaces found</p>
  }

  return (
    <div className="ns-selector">
      <label htmlFor="ns-select">Namespace</label>
      <select
        id="ns-select"
        value={selected}
        onChange={(e) => onChange(e.target.value)}
        disabled={disabled}
      >
        <option value="">— select —</option>
        {namespaces.map((ns) => (
          <option key={ns} value={ns}>{ns}</option>
        ))}
      </select>
    </div>
  )
}
