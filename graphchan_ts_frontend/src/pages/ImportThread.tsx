import React, { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { api } from '../api/client';
import './ImportThread.css';
import { Download, AlertTriangle, CheckCircle } from 'lucide-react';

export const ImportThread: React.FC = () => {
    const [url, setUrl] = useState('');
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const navigate = useNavigate();

    const handleSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!url.trim()) return;

        setLoading(true);
        setError(null);

        try {
            const response = await api.importThread(url);
            navigate(`/threads/${response.id}`);
        } catch (err) {
            console.error('Import failed:', err);
            setError('Failed to import thread. Please check the URL and try again.');
            setLoading(false);
        }
    };

    return (
        <div className="import-container">
            <div className="import-header">
                <h1 className="import-title">IMPORT THREAD</h1>
                <div className="import-subtitle">ARCHIVE EXTERNAL DATA</div>
            </div>

            <div className="import-card">
                <div className="import-icon-wrapper">
                    <Download className="import-icon" size={48} />
                </div>

                <p className="import-description">
                    Enter a 4chan thread URL to archive it locally.
                    Images and metadata will be downloaded and preserved.
                </p>

                <form onSubmit={handleSubmit} className="import-form">
                    <div className="input-group">
                        <label htmlFor="url">THREAD URL</label>
                        <input
                            id="url"
                            type="text"
                            value={url}
                            onChange={(e) => setUrl(e.target.value)}
                            placeholder="https://boards.4chan.org/g/thread/..."
                            disabled={loading}
                            autoFocus
                        />
                    </div>

                    {error && (
                        <div className="import-error">
                            <AlertTriangle size={16} />
                            <span>{error}</span>
                        </div>
                    )}

                    <button type="submit" className="import-button" disabled={loading || !url}>
                        {loading ? (
                            <span className="loading-text">IMPORTING...</span>
                        ) : (
                            <>
                                <CheckCircle size={18} />
                                <span>START IMPORT</span>
                            </>
                        )}
                    </button>
                </form>

                <div className="import-info">
                    <p>Supported boards: all SFW and NSFW boards.</p>
                    <p>Note: Large threads may take a moment to process.</p>
                </div>
            </div>
        </div>
    );
};
