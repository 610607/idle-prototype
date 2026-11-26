import { useLayoutEffect, useRef, useState, type CSSProperties, type ReactNode } from 'react';
import { createPortal } from 'react-dom';

interface GameTooltipProps {
  content: ReactNode;
  children: ReactNode;
  as?: 'span' | 'div';
  className?: string;
  style?: CSSProperties;
  disabled?: boolean;
}

export function GameTooltip({
  content,
  children,
  as = 'span',
  className,
  style,
  disabled = false,
}: GameTooltipProps) {
  const [visible, setVisible] = useState(false);
  const [cursor, setCursor] = useState<{ x: number; y: number }>({ x: 0, y: 0 });
  const [coords, setCoords] = useState<{ left: number; top: number }>({ left: -9999, top: -9999 });
  const boxRef = useRef<HTMLDivElement | null>(null);

  useLayoutEffect(() => {
    if (!visible || !boxRef.current || typeof window === 'undefined') return;
    const rect = boxRef.current.getBoundingClientRect();
    const pad = 10;
    const offset = 15;

    let left = cursor.x + offset;
    let top = cursor.y + offset;

    if (left + rect.width > window.innerWidth - pad) {
      left = cursor.x - rect.width - offset;
    }
    if (top + rect.height > window.innerHeight - pad) {
      top = cursor.y - rect.height - offset;
    }

    left = Math.max(pad, Math.min(left, window.innerWidth - rect.width - pad));
    top = Math.max(pad, Math.min(top, window.innerHeight - rect.height - pad));

    setCoords({ left, top });
  }, [visible, cursor, content]);

  const handleEnter = (e: React.PointerEvent) => {
    if (disabled || !content) return;
    setCursor({ x: e.clientX, y: e.clientY });
    setVisible(true);
  };

  const handleMove = (e: React.PointerEvent) => {
    if (!visible) return;
    setCursor({ x: e.clientX, y: e.clientY });
  };

  const handleLeave = () => {
    setVisible(false);
  };

  const Wrapper = as;

  const portal =
    visible && content && typeof document !== 'undefined'
      ? createPortal(
          <div
            ref={boxRef}
            className="rpg-tooltip-portal"
            style={{
              left: `${coords.left}px`,
              top: `${coords.top}px`,
            }}
          >
            {content}
          </div>,
          document.body,
        )
      : null;

  return (
    <>
      <Wrapper
        className={className}
        style={style}
        onPointerEnter={handleEnter}
        onPointerMove={handleMove}
        onPointerLeave={handleLeave}
      >
        {children}
      </Wrapper>
      {portal}
    </>
  );
}
