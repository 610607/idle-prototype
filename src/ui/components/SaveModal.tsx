import { useState } from 'react';
import { gameController } from '../../state/gameStore.ts';

interface SaveModalProps {
  open: boolean;
  onClose: () => void;
}

export function SaveModal({ open, onClose }: SaveModalProps) {
  const [exported, setExported] = useState('');
  const [importText, setImportText] = useState('');
  const [msg, setMsg] = useState<string | null>(null);

  if (!open) return null;

  const handleExport = () => {
    const str = gameController.exportSaveString();
    setExported(str);
    setMsg('已生成当前压缩存档字符串（IDLE_PROTO_V2）。');
  };

  const handleImport = () => {
    const res = gameController.importSaveString(importText);
    if (res.ok) {
      setMsg('导入成功！');
      onClose();
    } else {
      setMsg(res.error ?? '导入失败');
    }
  };

  const handleReset = () => {
    gameController.resetAllSave();
    onClose();
  };

  return (
    <div className="modal-backdrop">
      <div className="modal-box">
        <div className="panel-header">
          <h2>存档管理（lz-string + zod 校验）</h2>
          <button className="small" onClick={onClose}>
            关闭
          </button>
        </div>

        <div className="card">
          <div className="card-title-row">
            <strong>导出存档字符串</strong>
            <button className="small primary" onClick={handleExport}>
              生成导出串
            </button>
          </div>
          <textarea
            rows={4}
            readOnly
            value={exported}
            placeholder="点击「生成导出串」后可复制保存..."
          />
        </div>

        <div className="card">
          <div className="card-title-row">
            <strong>导入存档字符串</strong>
            <button
              className="small primary"
              disabled={!importText.trim()}
              onClick={handleImport}
            >
              校验并导入
            </button>
          </div>
          <textarea
            rows={4}
            value={importText}
            onChange={(e) => setImportText(e.target.value)}
            placeholder="粘贴 IDLE_PROTO_V2:... 字符串"
          />
        </div>

        {msg && <div style={{ color: 'var(--amber)' }}>{msg}</div>}

        <div style={{ display: 'flex', justifyContent: 'space-between', marginTop: '4px' }}>
          <button className="danger small" onClick={handleReset}>
            清空本地存档重开
          </button>
          <button onClick={onClose}>完成</button>
        </div>
      </div>
    </div>
  );
}
