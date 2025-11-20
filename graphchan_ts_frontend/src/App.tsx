import { BrowserRouter, Route, Routes } from 'react-router-dom';
import { Layout } from './components/Layout';
import { Dashboard } from './pages/Dashboard';
import { ThreadList } from './pages/ThreadList';
import { ThreadView } from './pages/ThreadView';
import { CreateThread } from './pages/CreateThread';
import { Identity } from './pages/Identity';
import { Placeholder } from './pages/Placeholder';

function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route path="/" element={<Layout />}>
          <Route index element={<Dashboard />} />
          <Route path="threads" element={<ThreadList />} />
          <Route path="threads/new" element={<CreateThread />} />
          <Route path="threads/:id" element={<ThreadView />} />
          <Route path="graph" element={<Placeholder title="GRAPH VISUALIZATION" />} />
          <Route path="peers" element={<Identity />} />
          <Route path="identity" element={<Identity />} />
          <Route path="*" element={<Placeholder title="404 - NOT FOUND" />} />
        </Route>
      </Routes>
    </BrowserRouter>
  );
}

export default App;
