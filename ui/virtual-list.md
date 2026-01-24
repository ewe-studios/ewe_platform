import React, { useState, useEffect, useRef } from 'react';

function VirtualizedList({ items, itemHeight }) {
  const [scrollTop, setScrollTop] = useState(0);
  const listRef = useRef(null);
  const containerHeight = 200; // Example container height

  const start = Math.floor(scrollTop / itemHeight);
  const end = Math.ceil((scrollTop + containerHeight) / itemHeight);

  const visibleItems = items.slice(start, end);

  useEffect(() => {
    if (listRef.current) {
      listRef.current.scrollTop = scrollTop;
    }
  }, [scrollTop]);

  return (
    <div
      style={{
        overflow: 'auto',
        height: containerHeight,
        position: 'relative', // Important for absolute positioning of items
      }}
      ref={listRef}
      onScroll={(e) => {
        setScrollTop(e.target.scrollTop);
      }}
    >
      <div
        style={{
          height: `${items.length * itemHeight}px`, // Total height of the list
          position: 'absolute',
          top: 0,
          left: 0,
        }}
      >
        {visibleItems.map((item, index) => (
          <div
            key={item.id}
            style={{
              position: 'absolute',
              top: `${(start + index) * itemHeight}px`,
              width: '100%',
              height: `${itemHeight}px`,
            }}
          >
            {/* Render the item */}
            {item.content}
          </div>
        ))}
      </div>
    </div>
  );
}
