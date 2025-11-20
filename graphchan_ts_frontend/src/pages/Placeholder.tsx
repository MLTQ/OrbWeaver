import React from 'react';

export const Placeholder: React.FC<{ title: string }> = ({ title }) => {
    return (
        <div className="flex flex-col items-center justify-center h-full text-[var(--text-secondary)]">
            <h2 className="text-2xl mb-4 text-[var(--accent-primary)]">{title}</h2>
            <div className="p-4 border border-[var(--border-color)] bg-[var(--bg-secondary)]">
                MODULE UNDER CONSTRUCTION
            </div>
        </div>
    );
};
