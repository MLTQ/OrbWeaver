import React, { useEffect, useState } from 'react';
import { Copy, Globe, Plus, Shield, UserPlus } from 'lucide-react';
import { api } from '../api/client';
import { type PeerView, type HealthResponse } from '../api/types';
import './Identity.css';

export const Identity: React.FC = () => {
    const [peers, setPeers] = useState<PeerView[]>([]);
    const [health, setHealth] = useState<HealthResponse | null>(null);
    const [friendcode, setFriendcode] = useState('');
    const [loading, setLoading] = useState(true);
    const [adding, setAdding] = useState(false);

    const fetchData = async () => {
        try {
            const [peersData, healthData] = await Promise.all([
                api.listPeers(),
                api.getHealth(),
            ]);
            setPeers(peersData);
            setHealth(healthData);
        } catch (e) {
            console.error('Failed to fetch identity data', e);
        } finally {
            setLoading(false);
        }
    };

    useEffect(() => {
        fetchData();
    }, []);

    const handleAddPeer = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!friendcode.trim()) return;

        setAdding(true);
        try {
            await api.addPeer(friendcode);
            setFriendcode('');
            fetchData();
        } catch (e) {
            console.error('Failed to add peer', e);
            alert('Failed to add peer');
        } finally {
            setAdding(false);
        }
    };

    const copyToClipboard = (text: string) => {
        navigator.clipboard.writeText(text);
        // Could add a toast notification here
    };

    if (loading) {
        return <div className="loading-screen">SCANNING NETWORK...</div>;
    }

    return (
        <div className="identity-container">
            {/* Local Identity Section */}
            <div className="identity-card">
                <div className="card-header">
                    <Shield size={20} />
                    <h2>LOCAL IDENTITY</h2>
                </div>

                <div className="identity-details">
                    <div className="detail-group">
                        <label>GPG FINGERPRINT</label>
                        <div className="code-display">
                            {health?.identity.gpg_fingerprint || 'UNKNOWN'}
                            <button onClick={() => copyToClipboard(health?.identity.gpg_fingerprint || '')}>
                                <Copy size={14} />
                            </button>
                        </div>
                    </div>

                    <div className="detail-group">
                        <label>FRIEND CODE</label>
                        <div className="code-display highlight">
                            {health?.identity.friendcode || 'UNKNOWN'}
                            <button onClick={() => copyToClipboard(health?.identity.friendcode || '')}>
                                <Copy size={14} />
                            </button>
                        </div>
                    </div>

                    <div className="detail-group">
                        <label>IROH PEER ID</label>
                        <div className="code-display small">
                            {health?.identity.iroh_peer_id || 'UNKNOWN'}
                        </div>
                    </div>
                </div>
            </div>

            {/* Add Peer Section */}
            <div className="add-peer-section">
                <div className="section-title">
                    <UserPlus size={18} />
                    <h3>ESTABLISH CONNECTION</h3>
                </div>
                <form onSubmit={handleAddPeer} className="add-peer-form">
                    <input
                        type="text"
                        value={friendcode}
                        onChange={(e) => setFriendcode(e.target.value)}
                        placeholder="Enter Friend Code..."
                        className="peer-input"
                    />
                    <button type="submit" className="btn btn-cyan" disabled={adding}>
                        {adding ? 'CONNECTING...' : 'ADD PEER'}
                        {!adding && <Plus size={16} className="ml-2" />}
                    </button>
                </form>
            </div>

            {/* Peer List Section */}
            <div className="peers-list-section">
                <div className="section-title">
                    <Globe size={18} />
                    <h3>KNOWN PEERS ({peers.length})</h3>
                </div>

                <div className="peers-grid">
                    {peers.map((peer) => (
                        <div key={peer.id} className="peer-card">
                            <div className="peer-status online"></div>
                            <div className="peer-info">
                                <div className="peer-alias">{peer.alias || 'Unknown Peer'}</div>
                                <div className="peer-id" title={peer.iroh_peer_id}>
                                    {peer.iroh_peer_id?.substring(0, 12)}...
                                </div>
                                <div className="peer-meta">
                                    Trust: {peer.trust_state}
                                </div>
                            </div>
                        </div>
                    ))}

                    {peers.length === 0 && (
                        <div className="empty-peers">NO PEERS CONNECTED</div>
                    )}
                </div>
            </div>
        </div>
    );
};
