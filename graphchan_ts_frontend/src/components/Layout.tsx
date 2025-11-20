import React, { useEffect, useState } from 'react';
import { Link, Outlet, useLocation } from 'react-router-dom';
import { Activity, Download, Globe, Hash, LayoutGrid, Users } from 'lucide-react';
import { api } from '../api/client';
import { type HealthResponse } from '../api/types';
import './Layout.css';

export const Layout: React.FC = () => {
    const location = useLocation();
    const [health, setHealth] = useState<HealthResponse | null>(null);

    useEffect(() => {
        const fetchHealth = async () => {
            try {
                const data = await api.getHealth();
                setHealth(data);
            } catch (e) {
                console.error('Failed to fetch health', e);
            }
        };
        fetchHealth();
        const interval = setInterval(fetchHealth, 10000);
        return () => clearInterval(interval);
    }, []);

    const navItems = [
        { path: '/', label: 'DASHBOARD', icon: LayoutGrid },
        { path: '/threads', label: 'THREADS', icon: Hash },
        { path: '/graph', label: 'GRAPH VIEW', icon: Activity },
        { path: '/import', label: 'IMPORT', icon: Download },
        { path: '/peers', label: 'NETWORK', icon: Globe },
        { path: '/identity', label: 'IDENTITY', icon: Users },
    ];

    return (
        <div className="layout-container">
            {/* Sidebar */}
            <aside className="sidebar">
                <div className="sidebar-header">
                    <h1 className="app-title">
                        ORBWEAVER
                    </h1>
                    <div className="system-ver">
                        SYSTEM_VER: {health?.version || 'UNKNOWN'}
                    </div>
                </div>

                <nav className="nav-menu">
                    {navItems.map((item) => {
                        const Icon = item.icon;
                        const isActive = location.pathname === item.path;
                        return (
                            <Link
                                key={item.path}
                                to={item.path}
                                className={`nav-link ${isActive ? 'active' : ''}`}
                            >
                                <Icon size={18} />
                                <span className="nav-label">{item.label}</span>
                            </Link>
                        );
                    })}
                </nav>

                {/* Status Panel */}
                <div className="status-panel">
                    <div className="status-row">
                        <span className="status-label">STATUS</span>
                        <span className="status-value-online">ONLINE</span>
                    </div>
                    <div className="status-row">
                        <span className="status-label">PEERS</span>
                        <span className="status-value-count">
                            {health?.network.addresses.length || 0}
                        </span>
                    </div>
                    <div className="identity-section">
                        <div className="status-label mb-1">IDENTITY_FP</div>
                        <div className="identity-fp" title={health?.identity.gpg_fingerprint}>
                            {health?.identity.gpg_fingerprint || 'LOADING...'}
                        </div>
                    </div>
                </div>
            </aside>

            {/* Main Content */}
            <main className="main-content">
                {/* Header/Top Bar */}
                <header className="top-bar">
                    <div className="breadcrumb">
                        <span className="breadcrumb-root">root@orbweaver</span>
                        <span>:</span>
                        <span>{location.pathname}</span>
                    </div>
                    <div className="time-display">
                        <span className="status-label">
                            {new Date().toISOString()}
                        </span>
                    </div>
                </header>

                {/* Content Area */}
                <div className="content-area">
                    {/* Grid Background */}
                    <div className="grid-bg" />
                    <div className="page-container">
                        <Outlet />
                    </div>
                </div>
            </main>
        </div>
    );
};
